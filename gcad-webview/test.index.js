console.log("Loading...");

import * as wasm from './pkg';

var test_program = `
board_thickness = 12.2mm;
board_height = 812.5mm - 6.35mm/2;
board_width = 5in;

groove_width = 17.3mm;
groove_depth = 6.1mm;
groove_x = 1in;

cutter_diameter(6.35mm);
material('BALTIC_BIRCH_PLYWOOD');

comment('Holes for threaded inserts for ceiling brackets');
for y in linspace(1.5in, board_height - 1.5in, 2) {
	for x in linspace(3/4in, 3.25in, 2) {
		comment('Counterbore');
		circle_pocket(x, y, radius=6.35mm, depth=3mm);
		comment('Threaded insert hole');
		circle_pocket(x, y, radius=4.75mm, depth=board_thickness);
	}
}


// Hole which the LED strip wires pass through
material('ALUMINUM');
comment('LED strip wire hole');
drill(groove_x + groove_width / 2, board_height - 1mm, board_thickness);
`;

const preview = new wasm.ToolpathPreview();

preview.update_gcad(test_program);