// "adi_gpu_opengl" - Aldaron's Device Interface / GPU / OpenGL
//
// Copyright Jeron A. Lau 2018.
// Distributed under the Boost Software License, Version 1.0.
// (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)

#version 100
precision mediump float;

attribute vec4 position;
attribute vec4 texpos;
attribute vec4 acolor;

uniform mat4 models_tfm; // The Models' Transform Matrix
uniform int has_camera; // 0 no, 1 yes, 2 fog
uniform mat4 matrix; // The Camera's Transform & Projection Matrix

varying vec4 vcolor;
varying vec4 texcoord;
varying float z;

void main() {
	vec4 place = models_tfm * vec4(position.xyz, 1.0);

	if(has_camera == 1) {
		place = matrix * place;
	}

	gl_Position = vec4(place.x, -place.y, place.z, place.w);
	vcolor = acolor;
	texcoord = vec4(texpos.xyz, texpos.w);
	z = length(gl_Position.xyz);
}
