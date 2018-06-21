// "adi_gpu_opengl" - Aldaron's Device Interface / GPU / OpenGL
//
// Copyright Jeron A. Lau 2018.
// Distributed under the Boost Software License, Version 1.0.
// (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)

#version 100
precision mediump float;

uniform sampler2D texture;

varying vec4 texcoord;
varying vec4 vcolor;

uniform int has_fog; // 0 no, 1 yes
uniform vec4 fog; // The fog color.
uniform vec2 range; // The range of fog (fog to far clip)

varying float z;

void main() {
	vec4 sampled = texture2D(texture, texcoord.xy);
	vec4 out_color = vec4(sampled.rgb, sampled.a * texcoord.a) * vcolor;

	if(has_fog == 1) {
		// Fog Calculation
		float linear = clamp((z-range.x) / range.y, 0.0, 1.0);
		float curved = linear * linear * linear;
		gl_FragColor = mix(out_color, fog, curved);
	} else {
		gl_FragColor = out_color;
	}
}
