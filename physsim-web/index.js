import * as wasm from "physsim-viz-rust";

//wasm.greet();
let runner = new wasm.Runner();

//Don't drop Runner instance
setInterval(() => {
	let _ = runner;
}, 1000);
