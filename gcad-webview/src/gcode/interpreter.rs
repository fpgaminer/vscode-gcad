use std::collections::HashMap;

use nalgebra::Point3;

use crate::gcode::parser::GcodeNumber;

use super::parser::{GcodeLetter, GcodeLine, GcodeWord};


/// Take a parsed Gcode program and interpret it.
/// The result is a list of motions.
pub fn run(program: &[GcodeLine]) -> Result<Vec<InterpreterMotion>, InterpreterError> {
	let mut motions = Vec::new();
	let mut state = HashMap::new();
	let mut motion_state = MotionState::Rapid;
	let mut distance_mode = DistanceMode::Absolute;

	state.insert(GcodeLetter::X, GcodeNumber::Float(0.0));
	state.insert(GcodeLetter::Y, GcodeNumber::Float(0.0));
	state.insert(GcodeLetter::Z, GcodeNumber::Float(0.0));
	state.insert(GcodeLetter::F, GcodeNumber::Float(0.0));
	state.insert(GcodeLetter::I, GcodeNumber::Float(0.0));
	state.insert(GcodeLetter::J, GcodeNumber::Float(0.0));

	for (line_num, GcodeLine { words }) in program.iter().enumerate() {
		let mut new_state = HashMap::new();

		// Non-Modals
		let mut g53_enabled = false;

		if words.len() == 0 {
			continue;
		}

		// Process each word
		// NOTE: It's probably an error to have multiple words from the same modal group on the same line, but we plow forward anyway and prefer the last one
		for GcodeWord { letter, number } in words {
			match (letter, number) {
				(GcodeLetter::G, GcodeNumber::Int(0)) => motion_state = MotionState::Rapid,
				(GcodeLetter::G, GcodeNumber::Int(1)) => motion_state = MotionState::Linear,
				(GcodeLetter::G, GcodeNumber::Int(2)) => motion_state = MotionState::ClockwiseArc,
				(GcodeLetter::G, GcodeNumber::Int(3)) => motion_state = MotionState::CounterClockwiseArc,
				(GcodeLetter::G, GcodeNumber::Int(21)) => {}, // TODO: Units (mm)
				(GcodeLetter::G, GcodeNumber::Int(53)) => g53_enabled = true,
				(GcodeLetter::G, GcodeNumber::Int(90)) => distance_mode = DistanceMode::Absolute,
				(GcodeLetter::G, GcodeNumber::Int(91)) => distance_mode = DistanceMode::Relative,
				(GcodeLetter::M, GcodeNumber::Int(2)) => return Ok(motions), // End of program
				(GcodeLetter::M, GcodeNumber::Int(3)) => (),                 // TODO: Spindle on clockwise
				(GcodeLetter::M, GcodeNumber::Int(5)) => {},                 // TODO: Spindle stop
				(GcodeLetter::G | GcodeLetter::M, _) => return Err(InterpreterError::new(InterpreterErrorReason::UnknownCommand, line_num + 1)),
				(&w, &v) => {
					new_state.insert(w, v);
				},
			}
		}

		if g53_enabled {
			// TODO
			continue;
		}

		// Perform motion if X, Y, or Z were present
		if new_state.contains_key(&GcodeLetter::X) || new_state.contains_key(&GcodeLetter::Y) || new_state.contains_key(&GcodeLetter::Z) {
			// Current position
			let x0: f32 = state.get(&GcodeLetter::X).unwrap().into();
			let y0: f32 = state.get(&GcodeLetter::Y).unwrap().into();
			let z0: f32 = state.get(&GcodeLetter::Z).unwrap().into();

			// Destination
			let x1: f32 = new_state.get(&GcodeLetter::X).map(|x| x.into()).unwrap_or(x0);
			let y1: f32 = new_state.get(&GcodeLetter::Y).map(|y| y.into()).unwrap_or(y0);
			let z1: f32 = new_state.get(&GcodeLetter::Z).map(|z| z.into()).unwrap_or(z0);

			// Center of arc
			let i: f32 = new_state.get(&GcodeLetter::I).unwrap_or(state.get(&GcodeLetter::I).unwrap()).into();
			let j: f32 = new_state.get(&GcodeLetter::J).unwrap_or(state.get(&GcodeLetter::J).unwrap()).into();

			// Feedrate
			let f: f32 = new_state.get(&GcodeLetter::F).unwrap_or(state.get(&GcodeLetter::F).unwrap()).into();

			let start = Point3::new(x0, y0, z0);
			let end = match distance_mode {
				DistanceMode::Absolute => Point3::new(x1, y1, z1),
				DistanceMode::Relative => Point3::new(x0 + x1, y0 + y1, z0 + z1),
			};
			let center = Point3::new(x0 + i, y0 + j, z0);

			let motion_type = match motion_state {
				MotionState::Rapid => MotionType::Rapid,
				MotionState::Linear => MotionType::Linear,
				MotionState::ClockwiseArc => MotionType::ClockwiseArc,
				MotionState::CounterClockwiseArc => MotionType::CounterClockwiseArc,
			};

			if motion_type == MotionType::ClockwiseArc || motion_type == MotionType::CounterClockwiseArc {
				let radius0 = (start.xy() - center.xy()).magnitude();
				let radius1 = (end.xy() - center.xy()).magnitude();
				let radius_diff = (radius0 - radius1).abs();

				if (radius_diff > 0.5) || (radius_diff > 0.005 && (radius_diff > (0.001 * radius0))) {
					return Err(InterpreterError::new(InterpreterErrorReason::BadArc, line_num + 1));
				}
			}

			motions.push(InterpreterMotion {
				motion_type,
				start,
				end,
				center,
				feed: if motion_state == MotionState::Rapid { None } else { Some(f) },
			});
		}

		// Update state
		state.extend(new_state);
	}

	Ok(motions)
}


#[derive(Debug)]
pub struct InterpreterError {
	reason: InterpreterErrorReason,
	line_number: usize,
}

impl InterpreterError {
	fn new(reason: InterpreterErrorReason, line_number: usize) -> Self {
		Self { reason, line_number }
	}
}

impl std::fmt::Display for InterpreterError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "Interpreter error: {} at line {}", self.reason, self.line_number)
	}
}

impl std::error::Error for InterpreterError {}


#[derive(Debug)]
pub enum InterpreterErrorReason {
	UnknownCommand,
	/// The radius of the arc is not constant
	BadArc,
}

impl std::fmt::Display for InterpreterErrorReason {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			InterpreterErrorReason::UnknownCommand => write!(f, "Unknown command"),
			InterpreterErrorReason::BadArc => write!(f, "The radius of the arc is not constant"),
		}
	}
}


#[derive(PartialEq)]
enum MotionState {
	Rapid,
	Linear,
	ClockwiseArc,
	CounterClockwiseArc,
}

enum DistanceMode {
	Absolute,
	Relative,
}


pub struct InterpreterMotion {
	pub motion_type: MotionType,
	pub start: Point3<f32>,
	pub end: Point3<f32>,
	pub center: Point3<f32>,
	/// Feedrate in mm/min.  If None, this is a rapid motion.
	pub feed: Option<f32>,
}


#[derive(PartialEq)]
pub enum MotionType {
	Rapid,
	Linear,
	ClockwiseArc,
	CounterClockwiseArc,
}
