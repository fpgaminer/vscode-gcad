use std::str::FromStr;


/// Parses a Gcode program into a list of GcodeLines.
/// A GcodeLine will be generated for every line of the input. Useful for tracking line numbers for interpreter errors.
pub fn parse<I: Iterator<Item = char>>(mut input: I) -> Result<Vec<GcodeLine>, ParserError> {
	let mut state = ParserState::None;
	let mut lines = Vec::new();
	let mut words = Vec::new();
	let mut value = String::new();
	let mut line_num = 1;
	let mut next_char = None;

	loop {
		let c = if let Some(c) = next_char.take() {
			c
		} else {
			match input.next() {
				Some('\r') => Some(' '),
				c => c,
			}
		};

		// Ignore whitespace
		if c == Some(' ') || c == Some('\t') {
			continue;
		}

		match (state, c) {
			(ParserState::None, Some('\n') | None) => {
				lines.push(GcodeLine {
					words: words.drain(..).collect(),
				});
				line_num += 1;

				if c.is_none() {
					break;
				}
			},
			(ParserState::None, Some('(')) => state = ParserState::EatingComment,
			(ParserState::None, Some(c)) => {
				let letter = GcodeLetter::try_from(c).map_err(|reason| ParserError::new(reason, line_num))?;
				state = ParserState::ReadingValue(letter);
				value.clear();
			},
			(ParserState::EatingComment, Some('\n') | None) => return Err(ParserError::new(ParserErrorReason::ExpectedEndOfComment, line_num)),
			(ParserState::EatingComment, Some(')')) => state = ParserState::None,
			(ParserState::EatingComment, _) => (),
			(ParserState::ReadingValue(_), Some(c)) if c.is_ascii_digit() || c == '.' || c == '-' => {
				value.push(c);
			},
			(ParserState::ReadingValue(_), _) if value.len() == 0 => {
				return Err(ParserError::new(ParserErrorReason::ExpectedNumber, line_num));
			},
			(ParserState::ReadingValue(letter), c) => {
				let number = value.parse().map_err(|reason| ParserError::new(reason, line_num))?;
				words.push(GcodeWord { letter, number });
				state = ParserState::None;

				// Process the character we just read
				next_char = Some(c);
			},
		}
	}

	Ok(lines)
}


#[derive(Debug)]
pub struct ParserError {
	reason: ParserErrorReason,
	line: usize,
}

impl ParserError {
	fn new(reason: ParserErrorReason, line: usize) -> Self {
		Self { reason, line }
	}
}

impl std::fmt::Display for ParserError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "Parser error: {} at line {}", self.reason, self.line)
	}
}

impl std::error::Error for ParserError {}


#[derive(Debug)]
pub enum ParserErrorReason {
	/// Parser expected a valid Gcode letter, but found something else
	ExpectedLetter,
	/// Parser expected a valid Gcode number, but found something else
	ExpectedNumber,
	ExpectedEndOfComment,
}

impl std::fmt::Display for ParserErrorReason {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Self::ExpectedLetter => write!(f, "Expected a valid Gcode letter"),
			Self::ExpectedNumber => write!(f, "Expected a valid Gcode number"),
			Self::ExpectedEndOfComment => write!(f, "Expected end of comment"),
		}
	}
}


#[derive(Copy, Clone)]
enum ParserState {
	None,
	ReadingValue(GcodeLetter),
	EatingComment,
}


#[derive(Clone, Copy)]
pub enum GcodeNumber {
	Int(i64),
	Float(f64),
}

impl From<GcodeNumber> for f32 {
	fn from(num: GcodeNumber) -> Self {
		match num {
			GcodeNumber::Int(i) => i as f32,
			GcodeNumber::Float(f) => f as f32,
		}
	}
}

impl From<&GcodeNumber> for f32 {
	fn from(num: &GcodeNumber) -> Self {
		match num {
			GcodeNumber::Int(i) => *i as f32,
			GcodeNumber::Float(f) => *f as f32,
		}
	}
}

impl FromStr for GcodeNumber {
	type Err = ParserErrorReason;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		if let Ok(int) = input.parse() {
			Ok(GcodeNumber::Int(int))
		} else if let Ok(float) = input.parse() {
			Ok(GcodeNumber::Float(float))
		} else {
			Err(ParserErrorReason::ExpectedNumber)
		}
	}
}

#[derive(Clone, Copy)]
pub struct GcodeWord {
	pub letter: GcodeLetter,
	pub number: GcodeNumber,
}


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum GcodeLetter {
	A,
	B,
	C,
	D,
	F,
	G,
	H,
	I,
	J,
	K,
	L,
	M,
	P,
	Q,
	R,
	S,
	T,
	U,
	V,
	W,
	X,
	Y,
	Z,
}

impl TryFrom<char> for GcodeLetter {
	type Error = ParserErrorReason;

	fn try_from(value: char) -> Result<Self, Self::Error> {
		match value {
			'a' | 'A' => Ok(GcodeLetter::A),
			'b' | 'B' => Ok(GcodeLetter::B),
			'c' | 'C' => Ok(GcodeLetter::C),
			'd' | 'D' => Ok(GcodeLetter::D),
			'f' | 'F' => Ok(GcodeLetter::F),
			'g' | 'G' => Ok(GcodeLetter::G),
			'h' | 'H' => Ok(GcodeLetter::H),
			'i' | 'I' => Ok(GcodeLetter::I),
			'j' | 'J' => Ok(GcodeLetter::J),
			'k' | 'K' => Ok(GcodeLetter::K),
			'l' | 'L' => Ok(GcodeLetter::L),
			'm' | 'M' => Ok(GcodeLetter::M),
			'p' | 'P' => Ok(GcodeLetter::P),
			'q' | 'Q' => Ok(GcodeLetter::Q),
			'r' | 'R' => Ok(GcodeLetter::R),
			's' | 'S' => Ok(GcodeLetter::S),
			't' | 'T' => Ok(GcodeLetter::T),
			'u' | 'U' => Ok(GcodeLetter::U),
			'v' | 'V' => Ok(GcodeLetter::V),
			'w' | 'W' => Ok(GcodeLetter::W),
			'x' | 'X' => Ok(GcodeLetter::X),
			'y' | 'Y' => Ok(GcodeLetter::Y),
			'z' | 'Z' => Ok(GcodeLetter::Z),
			_ => Err(ParserErrorReason::ExpectedLetter),
		}
	}
}


pub struct GcodeLine {
	pub words: Vec<GcodeWord>,
}
