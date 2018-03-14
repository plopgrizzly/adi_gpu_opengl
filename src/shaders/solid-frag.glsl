// shaders/solid-frag.glsl -- Aldaron's Device Interface / GPU / OpenGL
// Copyright (c) 2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>
// Licensed under the MIT LICENSE

#version 100
precision mediump float;

/*uniform int has_fog; // 0 no, 1 yes
uniform vec4 fog; // The fog color.
uniform vec2 range; // The range of fog (fog to far clip)

varying vec4 vcolor;
varying float z;

void main() {
	vec4 out_color = vec4(vcolor.rgba);

	if(has_fog == 1) {
		// Fog Calculation
		float linear = clamp((z-range.x) / range.y, 0.0, 1.0);
		float curved = linear * linear * linear;
		gl_FragColor = mix(out_color, fog, curved);
	} else {
		gl_FragColor = out_color;
	}*/
void main() {
	gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);// out_color;
}
 