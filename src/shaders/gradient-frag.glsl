// "adi_gpu_opengl" crate - Licensed under the MIT LICENSE
//  * Copyright (c) 2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>

#version 100
precision mediump float;

varying vec4 vcolor;

uniform int has_fog; // 0 no, 1 yes
uniform vec4 fog; // The fog color.
uniform vec2 range; // The range of fog (fog to far clip)

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
	}
}
 