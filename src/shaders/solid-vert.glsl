// shaders/solid-vert.glsl -- Aldaron's Device Interface / GPU / OpenGL
// Copyright (c) 2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>
// Licensed under the MIT LICENSE

#version 100
precision mediump float;

attribute vec4 position;

uniform mat4 models_tfm; // The Models' Transform Matrix
uniform int has_camera; // 0 no, 1 yes, 2 fog
uniform mat4 matrix; // The Camera's Transform & Projection Matrix

varying float z;

void main() {
	vec4 place = models_tfm * vec4(position.xyz, 1.0);

	if(has_camera == 1) {
		gl_Position = matrix * place;
	} else {
		gl_Position = place;
	}

	z = length(gl_Position.xyz);
}
