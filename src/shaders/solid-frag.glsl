// Copyright Jeron A. Lau 2018.
// Dual-licensed under either the MIT License or the Boost Software License,
// Version 1.0.  (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)

#version 100
precision mediump float;

uniform int has_fog; // 0 no, 1 yes
uniform vec4 fog; // The fog color.
uniform vec2 range; // The range of fog (fog to far clip)
uniform vec4 color;

varying float z;

void main() {
	if(has_fog == 1) {
		// Fog Calculation
		float linear = clamp((z-range.x) / range.y, 0.0, 1.0);
		float curved = linear * linear * linear;
		gl_FragColor = mix(color, fog, curved);
	} else {
		gl_FragColor = color;
	}
}
