// TODO: Fix the buggy view when orbiting below the plane
// TODO: View cube?
mod camera;
mod gcode;

use std::{
	io::Cursor,
	sync::{Arc, Mutex},
};

use gcode::interpreter::MotionType;
use libgcad::BUILTIN_MATERIALS;
use nalgebra::{Dim, IsContiguous, Matrix, Point3, RawStorage, Rotation2, Vector3};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{console, WebGl2RenderingContext, WebGlProgram, WebGlShader};

use libgcad::ScriptEngine;

use gcode::interpreter::InterpreterMotion;


#[wasm_bindgen]
pub struct ToolpathPreview {
	vscode: Option<js_sys::Object>,
	canvas: web_sys::HtmlCanvasElement,
	gl: WebGl2RenderingContext,
	shader: WebGlProgram,
	gcad_source: String,
	gcode_source: String,
	toolpaths: Vec<InterpreterMotion>,
	toolpath_verts: Vec<Point3<f32>>,
	toolpath_colors: Vec<Point3<f32>>,

	position_buffer: web_sys::WebGlBuffer,
	color_buffer: web_sys::WebGlBuffer,
	toolpath_vao: web_sys::WebGlVertexArrayObject,

	grid_position_buffer: web_sys::WebGlBuffer,
	grid_color_buffer: web_sys::WebGlBuffer,
	grid_vao: web_sys::WebGlVertexArrayObject,

	camera: camera::Camera,

	mouse_state: MouseState,

	render_closure: Option<Closure<dyn FnMut(f64)>>,

	dirty: bool,
}


#[wasm_bindgen]
pub struct ToolpathPreviewMutex {
	inner: Arc<Mutex<ToolpathPreview>>,
}

#[wasm_bindgen]
impl ToolpathPreviewMutex {
	pub fn update_gcad(&mut self, gcad_source: &str) -> Result<(), JsValue> {
		let mut inner = self.inner.lock().unwrap();
		inner.update_gcad(gcad_source)
	}
}


enum MouseState {
	None,
	Orbit { x: f32, y: f32 },
	Pan { x: f32, y: f32 },
}


#[wasm_bindgen]
impl ToolpathPreview {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Result<ToolpathPreviewMutex, JsValue> {
		let window = web_sys::window().unwrap();
		let document = window.document().unwrap();
		let canvas = document.get_element_by_id("toolpath-canvas").unwrap();
		let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into()?;
		let global = js_sys::global();
		let vscode = if js_sys::Reflect::has(&global, &JsValue::from_str("acquireVsCodeApi"))? {
			let func = js_sys::Reflect::get(&global, &JsValue::from_str("acquireVsCodeApi"))?.dyn_into::<js_sys::Function>()?;
			Some(func.call0(&JsValue::NULL)?.dyn_into::<js_sys::Object>()?)
		} else {
			None
		};

		canvas.set_width(canvas.client_width() as u32);
		canvas.set_height(canvas.client_height() as u32);

		let gl = canvas.get_context("webgl2")?.unwrap().dyn_into::<WebGl2RenderingContext>()?;

		let vert_shader = compile_shader(
			&gl,
			WebGl2RenderingContext::VERTEX_SHADER,
			r##"#version 300 es
		
			in vec4 position;
			in vec3 color;
			uniform mat4 projectionMatrix;
	
			out vec4 vColor;
	
			void main() {
				gl_Position = projectionMatrix * vec4(position.x, position.y, position.z, 1.0);
				vColor = vec4(color.x, color.y, color.z, 1.0);
			}
			"##,
		)?;

		let frag_shader = compile_shader(
			&gl,
			WebGl2RenderingContext::FRAGMENT_SHADER,
			r##"#version 300 es
		
			precision highp float;
			in vec4 vColor;
			out vec4 outColor;
			
			void main() {
				outColor = vColor;
			}
			"##,
		)?;
		let shader = link_program(&gl, &vert_shader, &frag_shader)?;

		let position_buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
		let color_buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
		let toolpath_vao = gl.create_vertex_array().ok_or("Failed to create VAO")?;

		let grid_position_buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
		let grid_color_buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
		let grid_vao = gl.create_vertex_array().ok_or("Failed to create VAO")?;

		let toolpath_preview = Arc::new(Mutex::new(ToolpathPreview {
			vscode,
			canvas: canvas.clone(),
			gl,
			shader,
			gcad_source: String::new(),
			gcode_source: String::new(),
			toolpaths: Vec::new(),
			toolpath_verts: Vec::new(),
			toolpath_colors: Vec::new(),
			position_buffer,
			color_buffer,
			toolpath_vao,
			grid_position_buffer,
			grid_color_buffer,
			grid_vao,
			camera: camera::Camera::new(
				Point3::new(-1000.0, -1000.0, 1000.0),
				Point3::new(0.0, 0.0, 0.0),
				0.1,
				canvas.width() as f32 / canvas.height() as f32,
			),
			mouse_state: MouseState::None,
			render_closure: None,
			dirty: true,
		}));

		let closure_toolpath_preview = toolpath_preview.clone();
		let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
			closure_toolpath_preview.lock().unwrap().on_mouse_move(&event);
		});
		canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
		closure.forget();

		let closure_toolpath_preview = toolpath_preview.clone();
		let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
			closure_toolpath_preview.lock().unwrap().on_mouse_down(&event);
		});
		canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
		closure.forget();

		let closure_toolpath_preview = toolpath_preview.clone();
		let closure = Closure::<dyn FnMut(_) -> bool>::new(move |event: web_sys::WheelEvent| {
			event.prevent_default();
			closure_toolpath_preview.lock().unwrap().on_mouse_wheel(&event);
			false
		});
		canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
		closure.forget();

		// Disable context menu so we can use right click for panning
		let closure = Closure::<dyn FnMut(_) -> bool>::new(move |event: web_sys::MouseEvent| {
			event.prevent_default();
			false
		});
		canvas.add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())?;
		closure.forget();

		// Request animation frame
		let closure_toolpath_preview = toolpath_preview.clone();
		let render_closure = Closure::<dyn FnMut(_)>::new(move |_: f64| {
			let mut toolpath_preview = closure_toolpath_preview.lock().unwrap();

			toolpath_preview.render();

			if let Some(render_closure) = toolpath_preview.render_closure.as_ref() {
				web_sys::window()
					.unwrap()
					.request_animation_frame(render_closure.as_ref().unchecked_ref())
					.unwrap();
			}
		});
		toolpath_preview.lock().unwrap().render_closure = Some(render_closure);
		web_sys::window()
			.unwrap()
			.request_animation_frame(toolpath_preview.lock().unwrap().render_closure.as_ref().unwrap().as_ref().unchecked_ref())?;

		// onMessage
		let closure_toolpath_preview = toolpath_preview.clone();
		let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
			closure_toolpath_preview.lock().unwrap().on_message(&event);
		});
		window.add_event_listener_with_callback("message", closure.as_ref().unchecked_ref())?;
		closure.forget();

		Ok(ToolpathPreviewMutex { inner: toolpath_preview })
	}

	pub fn update_gcad(&mut self, program: &str) -> Result<(), JsValue> {
		self.gcad_source = program.to_string();

		// Compile into GCode
		self.gcode_source = match compile_gcad(program) {
			Ok(gcode) => gcode,
			Err(err) => {
				self.post_error(&format!("Error compiling GCad program: {}", err));
				return Ok(());
			},
		};

		console::log_1(&format!("Compiled Gcode: {}", self.gcode_source).into());

		// Parse GCode into toolpaths
		self.toolpaths = match gcode_to_toolpaths(&self.gcode_source) {
			Ok(toolpaths) => toolpaths,
			Err(err) => {
				self.post_error(&format!("Error converting Gcode to toolpaths: {:?}", err));
				return Ok(());
			},
		};

		// Convert toolpaths into vertex data
		(self.toolpath_verts, self.toolpath_colors) = toolpaths_to_gl(&self.toolpaths);
		let verts = vertices_to_floats(self.toolpath_verts.iter().map(|v| &v.coords));
		let colors = vertices_to_floats(self.toolpath_colors.iter().map(|v| &v.coords));

		// Upload vertex data to GPU
		let position_attribute_location = self.gl.get_attrib_location(&self.shader, "position");
		let color_attribute_location = self.gl.get_attrib_location(&self.shader, "color");

		copy_data_to_gl_buffer(&self.gl, &self.position_buffer, &verts);
		copy_data_to_gl_buffer(&self.gl, &self.color_buffer, &colors);

		self.gl.bind_vertex_array(Some(&self.toolpath_vao));

		self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.position_buffer));
		self.gl.enable_vertex_attrib_array(position_attribute_location as u32);
		self.gl
			.vertex_attrib_pointer_with_i32(position_attribute_location as u32, 3, WebGl2RenderingContext::FLOAT, false, 0, 0);

		self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.color_buffer));
		self.gl.enable_vertex_attrib_array(color_attribute_location as u32);
		self.gl
			.vertex_attrib_pointer_with_i32(color_attribute_location as u32, 3, WebGl2RenderingContext::FLOAT, false, 0, 0);

		// Render
		self.dirty = true;

		Ok(())
	}

	fn render(&mut self) {
		if self.canvas.width() != self.canvas.client_width() as u32 || self.canvas.height() != self.canvas.client_height() as u32 {
			self.canvas.set_width(self.canvas.client_width() as u32);
			self.canvas.set_height(self.canvas.client_height() as u32);
			self.camera.set_aspect_ratio(self.canvas.width() as f32 / self.canvas.height() as f32);
			self.dirty = true;
		}

		if !self.dirty {
			return;
		}

		self.dirty = false;

		let projection_matrix_location = self.gl.get_uniform_location(&self.shader, "projectionMatrix");

		self.gl.viewport(0, 0, self.canvas.width() as i32, self.canvas.height() as i32);
		self.gl.clear_color(0.95, 0.95, 0.95, 1.0);
		self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
		self.gl.use_program(Some(&self.shader));

		// Set the projection matrix
		self.gl
			.uniform_matrix4fv_with_f32_array(projection_matrix_location.as_ref(), false, self.camera.view_projection().as_slice());

		self.gl.bind_vertex_array(Some(&self.toolpath_vao));

		self.gl.draw_arrays(WebGl2RenderingContext::LINES, 0, self.toolpath_verts.len() as i32);

		self.build_grid();

		loop {
			let err = self.gl.get_error();
			if err == WebGl2RenderingContext::NO_ERROR {
				break;
			}
			console::log_1(&format!("GL Error: {:?}", err).into());
		}
	}

	fn toolpath_extent(&self) -> (Vector3<f32>, Vector3<f32>) {
		let mut min_x = f32::MAX;
		let mut max_x = f32::MIN;
		let mut min_y = f32::MAX;
		let mut max_y = f32::MIN;
		let mut min_z = f32::MAX;
		let mut max_z = f32::MIN;
		for point in &self.toolpath_verts {
			min_x = min_x.min(point.x);
			max_x = max_x.max(point.x);
			min_y = min_y.min(point.y);
			max_y = max_y.max(point.y);
			min_z = min_z.min(point.z);
			max_z = max_z.max(point.z);
		}
		(Vector3::new(min_x, min_y, min_z), Vector3::new(max_x, max_y, max_z))
	}

	fn build_grid(&mut self) {
		// Calculate grid extents
		let inv_view_proj = self.camera.inverse_view_projection();
		let inv_view = self.camera.view().to_homogeneous().try_inverse().unwrap();
		let ray_z = (self.camera.z_near() + self.camera.z_far()) / (self.camera.z_near() - self.camera.z_far());
		let ray_origin0 = inv_view_proj.transform_point(&Point3::new(-1.0, 1.0, ray_z));
		let ray_origin1 = inv_view_proj.transform_point(&Point3::new(1.0, 1.0, ray_z));
		let ray_origin2 = inv_view_proj.transform_point(&Point3::new(-1.0, -1.0, ray_z));
		let ray_origin3 = inv_view_proj.transform_point(&Point3::new(1.0, -1.0, ray_z));
		let ray_direction = inv_view.transform_vector(&-Vector3::z());

		let denom = ray_direction.z;

		if denom == 0.0 {
			return;
		}

		let t = -ray_origin0.z / denom;
		let p0 = ray_origin0 + t * ray_direction;
		let t = -ray_origin1.z / denom;
		let p1 = ray_origin1 + t * ray_direction;
		let t = -ray_origin2.z / denom;
		let p2 = ray_origin2 + t * ray_direction;
		let t = -ray_origin3.z / denom;
		let p3 = ray_origin3 + t * ray_direction;

		let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
		let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
		let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
		let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);

		let large_grid = if self.camera.zoom() > 0.125 {
			1.0
		} else if self.camera.zoom() > 0.01 {
			10.0
		} else if self.camera.zoom() > 0.001 {
			100.0
		} else {
			1000.0
		};
		let small_grid = large_grid / 5.0;

		let mut verts = Vec::new();
		let mut colors = Vec::new();
		let large_grid_color = Vector3::new(0.7, 0.7, 0.7);
		let small_grid_color = Vector3::new(0.85, 0.85, 0.85);

		let start_x = (min_x / large_grid).floor() as i64;
		let end_x = (max_x / large_grid).ceil() as i64;

		for i in start_x..=end_x {
			let x = i as f32 * large_grid;
			verts.push(Vector3::new(x, min_y, 0.0));
			verts.push(Vector3::new(x, max_y, 0.0));
			colors.push(large_grid_color);
			colors.push(large_grid_color);

			for j in 1..5 {
				let sx = x + j as f32 * small_grid;
				verts.push(Vector3::new(sx, min_y, 0.0));
				verts.push(Vector3::new(sx, max_y, 0.0));
				colors.push(small_grid_color);
				colors.push(small_grid_color);
			}
		}

		let start_y = (min_y / large_grid).floor() as i64;
		let end_y = (max_y / large_grid).ceil() as i64;

		for i in start_y..=end_y {
			let y = i as f32 * large_grid;
			verts.push(Vector3::new(min_x, y, 0.0));
			verts.push(Vector3::new(max_x, y, 0.0));
			colors.push(large_grid_color);
			colors.push(large_grid_color);

			for j in 1..5 {
				let sy = y + j as f32 * small_grid;
				verts.push(Vector3::new(min_x, sy, 0.0));
				verts.push(Vector3::new(max_x, sy, 0.0));
				colors.push(small_grid_color);
				colors.push(small_grid_color);
			}
		}

		let verts = vertices_to_floats(verts.iter());
		let colors = vertices_to_floats(colors.iter());

		// Upload vertex data to GPU
		let position_attribute_location = self.gl.get_attrib_location(&self.shader, "position");
		let color_attribute_location = self.gl.get_attrib_location(&self.shader, "color");

		copy_data_to_gl_buffer(&self.gl, &self.grid_position_buffer, &verts);
		copy_data_to_gl_buffer(&self.gl, &self.grid_color_buffer, &colors);

		self.gl.bind_vertex_array(Some(&self.grid_vao));

		self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.grid_position_buffer));
		self.gl.enable_vertex_attrib_array(position_attribute_location as u32);
		self.gl
			.vertex_attrib_pointer_with_i32(position_attribute_location as u32, 3, WebGl2RenderingContext::FLOAT, false, 0, 0);

		self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.grid_color_buffer));
		self.gl.enable_vertex_attrib_array(color_attribute_location as u32);
		self.gl
			.vertex_attrib_pointer_with_i32(color_attribute_location as u32, 3, WebGl2RenderingContext::FLOAT, false, 0, 0);

		// Render
		self.gl.draw_arrays(WebGl2RenderingContext::LINES, 0, (verts.len() / 3) as i32);
	}

	fn on_mouse_down(&mut self, event: &web_sys::MouseEvent) {
		if event.buttons() & 1 != 0 {
			// Left mouse button
			self.mouse_state = MouseState::Pan {
				x: event.offset_x() as f32,
				y: event.offset_y() as f32,
			};
		} else if event.buttons() & 2 != 0 {
			// Right mouse button
			self.mouse_state = MouseState::Orbit {
				x: event.offset_x() as f32,
				y: event.offset_y() as f32,
			};
		}
	}

	fn on_mouse_move(&mut self, event: &web_sys::MouseEvent) {
		let x = event.offset_x() as f32;
		let y = event.offset_y() as f32;

		match self.mouse_state {
			MouseState::None => (),
			MouseState::Orbit { x: _, y: _ } if event.buttons() & 2 == 0 => self.mouse_state = MouseState::None,
			MouseState::Pan { x: _, y: _ } if event.buttons() & 1 == 0 => self.mouse_state = MouseState::None,
			MouseState::Orbit { x: old_x, y: old_y } => {
				let dx = x - old_x;
				let dy = y - old_y;

				let theta = 0.01 * dx;
				let phi = 0.01 * dy;

				self.camera.orbit(theta, phi, 0.0);

				self.mouse_state = MouseState::Orbit { x, y };

				self.dirty = true;
			},
			MouseState::Pan { x: old_x, y: old_y } => {
				let width = self.canvas.width() as f32;
				let height = self.canvas.height() as f32;

				self.camera.pan(
					(old_x / width) * 2.0 - 1.0,
					1.0 - (old_y / height) * 2.0,
					(x / width) * 2.0 - 1.0,
					1.0 - (y / height) * 2.0,
				);

				self.mouse_state = MouseState::Pan { x, y };

				self.dirty = true;
			},
		}
	}

	fn on_mouse_wheel(&mut self, event: &web_sys::WheelEvent) {
		let delta = event.delta_y() as f32;

		if delta > 0.0 {
			self.camera.set_zoom(self.camera.zoom() * 1.02);
		} else {
			self.camera.set_zoom(self.camera.zoom() / 1.02);
		}

		self.dirty = true;
	}

	fn on_message(&mut self, event: &web_sys::MessageEvent) {
		let program = event.data().as_string().unwrap();

		let _ = self.update_gcad(&program);
	}

	fn post_message(&self, message: JsValue) {
		if let Some(vscode) = &self.vscode {
			if let Err(err) = js_sys::Reflect::get(&vscode, &JsValue::from_str("postMessage"))
				.and_then(|f| f.dyn_into::<js_sys::Function>())
				.and_then(|f| f.call1(&vscode, &message))
			{
				console::log_1(&format!("Error posting message: {:?}", err).into());
			}
		}
	}

	fn post_error(&self, message: &str) {
		let obj = js_sys::Object::new();
		js_sys::Reflect::set(&obj, &JsValue::from_str("error"), &JsValue::from_str(message)).unwrap();
		self.post_message(obj.into());
	}
}


pub fn compile_shader(context: &WebGl2RenderingContext, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
	let shader = context
		.create_shader(shader_type)
		.ok_or_else(|| String::from("Unable to create shader object"))?;
	context.shader_source(&shader, source);
	context.compile_shader(&shader);

	if context
		.get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
		.as_bool()
		.unwrap_or(false)
	{
		Ok(shader)
	} else {
		Err(context
			.get_shader_info_log(&shader)
			.unwrap_or_else(|| String::from("Unknown error creating shader")))
	}
}


pub fn link_program(context: &WebGl2RenderingContext, vert_shader: &WebGlShader, frag_shader: &WebGlShader) -> Result<WebGlProgram, String> {
	let program = context.create_program().ok_or_else(|| String::from("Unable to create shader object"))?;

	context.attach_shader(&program, vert_shader);
	context.attach_shader(&program, frag_shader);
	context.link_program(&program);

	if context
		.get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
		.as_bool()
		.unwrap_or(false)
	{
		Ok(program)
	} else {
		Err(context
			.get_program_info_log(&program)
			.unwrap_or_else(|| String::from("Unknown error creating program object")))
	}
}


fn compile_gcad(program: &str) -> anyhow::Result<String> {
	let mut cursor = Cursor::new(Vec::new());
	let mut machine = ScriptEngine::new();
	machine.write_header();
	machine.run(BUILTIN_MATERIALS, false)?;
	machine.run(program, false)?;
	machine.finish(&mut cursor)?;

	let gcode = String::from_utf8(cursor.into_inner()).expect("Found invalid UTF-8");

	Ok(gcode)
}


fn gcode_to_toolpaths(program: &str) -> anyhow::Result<Vec<InterpreterMotion>> {
	let parsed_program = gcode::parse(program.chars())?;
	let toolpaths = gcode::interpreter::run(&parsed_program)?;

	Ok(toolpaths)
}


fn toolpaths_to_gl(toolpaths: &[InterpreterMotion]) -> (Vec<Point3<f32>>, Vec<Point3<f32>>) {
	let mut vertices = Vec::new();
	let mut colors = Vec::new();

	for toolpath in toolpaths {
		let color = if toolpath.feed.is_none() {
			Point3::new(1.0, 0.0, 0.0)
		} else {
			Point3::new(0.0, 1.0, 0.0)
		};

		match toolpath.motion_type {
			MotionType::Rapid | MotionType::Linear => {
				vertices.push(toolpath.start);
				vertices.push(toolpath.end);
				colors.push(color);
				colors.push(color);
			},
			MotionType::ClockwiseArc | MotionType::CounterClockwiseArc => {
				// The interpreter makes sure this arc has a reasonably constant radius already, so we don't need to check.
				let start_vector = toolpath.start.xy() - toolpath.center.xy();
				let end_vector = toolpath.end.xy() - toolpath.center.xy();
				let radius = start_vector.magnitude();
				let clockwise = toolpath.motion_type == MotionType::ClockwiseArc;

				// Calculate the angle we need to travel
				let angle =
					(start_vector.x * end_vector.y - start_vector.y * end_vector.x).atan2(start_vector.x * end_vector.x + start_vector.y * end_vector.y);
				let angle = if clockwise && angle >= 5e-7 {
					angle - 2.0 * std::f32::consts::PI
				} else if !clockwise && angle <= -5e-7 {
					angle + 2.0 * std::f32::consts::PI
				} else {
					angle
				};

				let tolerance = 0.01;
				let segments = ((0.5 * angle * radius).abs() / (tolerance * (2.0 * radius - tolerance)).sqrt()).floor() as usize;

				let mut position = toolpath.start;

				for i in 1..=segments {
					let theta = angle * i as f32 / segments as f32;
					let xy = toolpath.center.xy() + Rotation2::new(theta) * start_vector;
					let z = toolpath.start.z + (toolpath.end.z - toolpath.start.z) * i as f32 / segments as f32;

					vertices.push(position);
					position = Point3::new(xy.x, xy.y, z);
					vertices.push(position);
					colors.push(color);
					colors.push(color);
				}

				// One last segment to ensure we get to toolpath.end, in case of radius mismatch, etc.
				vertices.push(position);
				vertices.push(toolpath.end);
				colors.push(color);
				colors.push(color);
			},
		}
	}

	(vertices, colors)
}


fn vertices_to_floats<'a, T, R, C, S, I>(vertices: I) -> Vec<T>
where
	T: Clone + 'a,
	R: Dim,
	C: Dim,
	S: RawStorage<T, R, C> + IsContiguous + 'a,
	I: Iterator<Item = &'a Matrix<T, R, C, S>>,
{
	vertices.flat_map(|vertex| vertex.as_slice()).cloned().collect()
}


fn copy_data_to_gl_buffer(gl: &WebGl2RenderingContext, buffer: &web_sys::WebGlBuffer, data: &[f32]) {
	gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(buffer));
	// Note that `Float32Array::view` is somewhat dangerous (hence the
	// `unsafe`!). This is creating a raw view into our module's
	// `WebAssembly.Memory` buffer, but if we allocate more pages for ourself
	// (aka do a memory allocation in Rust) it'll cause the buffer to change,
	// causing the `Float32Array` to be invalid.
	//
	// As a result, after `Float32Array::view` we have to be very careful not to
	// do any memory allocations before it's dropped.
	unsafe {
		let positions_array_buf_view = js_sys::Float32Array::view(&data);

		gl.buffer_data_with_array_buffer_view(
			WebGl2RenderingContext::ARRAY_BUFFER,
			&positions_array_buf_view,
			WebGl2RenderingContext::STATIC_DRAW,
		);
	}
}
