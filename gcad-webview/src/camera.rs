use nalgebra::{Isometry3, Matrix4, Point3, Vector3};


#[derive(Debug, Clone)]
pub(crate) struct Camera {
	position: Point3<f32>,
	look_at: Point3<f32>,
	up: Vector3<f32>,
	zoom: f32,
	view_matrix: Option<Isometry3<f32>>,
	inverse_view_matrix: Option<Matrix4<f32>>,

	z_near: f32,
	z_far: f32,
	aspect_ratio: f32,
	projection_matrix: Option<Matrix4<f32>>,
	inverse_projection_matrix: Option<Matrix4<f32>>,

	view_projection_matrix: Option<Matrix4<f32>>,
	inverse_view_projection_matrix: Option<Matrix4<f32>>,
}

impl Camera {
	pub fn new(position: Point3<f32>, look_at: Point3<f32>, zoom: f32, aspect_ratio: f32) -> Self {
		Camera {
			position,
			look_at,
			up: Vector3::new(0.0, 0.0, 1.0),
			zoom,
			view_matrix: None,
			inverse_view_matrix: None,

			z_near: 0.1,
			z_far: 10000.0,
			aspect_ratio,
			projection_matrix: None,
			inverse_projection_matrix: None,

			view_projection_matrix: None,
			inverse_view_projection_matrix: None,
		}
	}

	fn invalidate_matrices(&mut self) {
		self.view_matrix = None;
		self.inverse_view_matrix = None;
		self.projection_matrix = None;
		self.inverse_projection_matrix = None;
		self.view_projection_matrix = None;
		self.inverse_view_projection_matrix = None;
	}

	pub fn orbit(&mut self, d_theta: f32, d_phi: f32, d_radius: f32) {
		let offset = self.position - self.look_at;
		let radius = offset.magnitude();
		// theta is measured from the positive y axis (x-y plane, 0 is at the positive y axis)
		let theta = f32::atan2(offset.x, offset.y);
		// phi is measured from the positive z axis
		let phi = (offset.z / radius).clamp(-1.0, 1.0).acos();

		let new_theta = theta + d_theta;
		let new_phi = (phi + d_phi).clamp(std::f32::EPSILON, std::f32::consts::PI - std::f32::EPSILON);
		let new_radius = (radius + d_radius).max(0.1);

		let x = new_radius * f32::sin(new_phi) * f32::sin(new_theta);
		let y = new_radius * f32::sin(new_phi) * f32::cos(new_theta);
		let z = new_radius * f32::cos(new_phi);

		self.position = self.look_at + Vector3::new(x, y, z);

		self.invalidate_matrices();
	}

	pub fn pan(&mut self, old_x: f32, old_y: f32, new_x: f32, new_y: f32) {
		let ray_origin0 = self
			.inverse_view_projection()
			.transform_point(&Point3::new(old_x, old_y, (self.z_near + self.z_far) / (self.z_near - self.z_far)));
		let ray_origin1 = self
			.inverse_view_projection()
			.transform_point(&Point3::new(new_x, new_y, (self.z_near + self.z_far) / (self.z_near - self.z_far)));
		let ray_direction = self.inverse_view().transform_vector(&-Vector3::z());

		let denom = Vector3::z().dot(&ray_direction);
		let pan = if denom == 0.0 {
			return;
		} else {
			let t = -ray_origin0.z / denom;
			let p0 = ray_origin0 + t * ray_direction;
			let t = -ray_origin1.z / denom;
			let p1 = ray_origin1 + t * ray_direction;
			p1 - p0
		};

		self.position -= pan;
		self.look_at -= pan;

		self.invalidate_matrices();
	}

	pub fn view(&mut self) -> Isometry3<f32> {
		*self
			.view_matrix
			.get_or_insert_with(|| Isometry3::look_at_rh(&self.position.into(), &self.look_at.into(), &self.up))
	}

	pub fn inverse_view(&mut self) -> Matrix4<f32> {
		if let Some(matrix) = self.inverse_view_matrix {
			matrix
		} else {
			let matrix = self.view().inverse().to_homogeneous();
			self.inverse_view_matrix = Some(matrix);
			matrix
		}
	}

	pub fn projection(&mut self) -> Matrix4<f32> {
		*self.projection_matrix.get_or_insert_with(|| {
			let dx = (2.0 * self.aspect_ratio) / (2.0 * self.zoom);
			let dy = (2.0) / (2.0 * self.zoom);

			Matrix4::new_orthographic(-dx, dx, -dy, dy, self.z_near, self.z_far)
		})
	}

	pub fn inverse_projection(&mut self) -> Matrix4<f32> {
		if let Some(matrix) = self.inverse_projection_matrix {
			matrix
		} else {
			let matrix = self.projection().try_inverse().unwrap();
			self.inverse_projection_matrix = Some(matrix);
			matrix
		}
	}

	pub fn view_projection(&mut self) -> Matrix4<f32> {
		if let Some(matrix) = self.view_projection_matrix {
			matrix
		} else {
			let matrix = self.projection() * self.view().to_homogeneous();
			self.view_projection_matrix = Some(matrix);
			matrix
		}
	}

	pub fn inverse_view_projection(&mut self) -> Matrix4<f32> {
		if let Some(matrix) = self.inverse_view_projection_matrix {
			matrix
		} else {
			let matrix = self.view_projection().try_inverse().unwrap();
			self.inverse_view_projection_matrix = Some(matrix);
			matrix
		}
	}

	pub fn z_near(&self) -> f32 {
		self.z_near
	}

	pub fn z_far(&self) -> f32 {
		self.z_far
	}

	pub fn zoom(&self) -> f32 {
		self.zoom
	}

	pub fn set_zoom(&mut self, zoom: f32) {
		self.zoom = zoom.clamp(0.0001, 5.0);
		self.invalidate_matrices();
	}

	pub fn aspect_ratio(&self) -> f32 {
		self.aspect_ratio
	}

	pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
		self.aspect_ratio = aspect_ratio;
		self.invalidate_matrices();
	}

	pub fn left(&self) -> f32 {
		-(2.0 * self.aspect_ratio) / (2.0 * self.zoom)
	}

	pub fn right(&self) -> f32 {
		(2.0 * self.aspect_ratio) / (2.0 * self.zoom)
	}

	pub fn top(&self) -> f32 {
		(2.0) / (2.0 * self.zoom)
	}

	pub fn bottom(&self) -> f32 {
		-(2.0) / (2.0 * self.zoom)
	}

	pub fn position(&self) -> Point3<f32> {
		self.position
	}
}
