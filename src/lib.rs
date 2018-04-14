// lib.rs -- Aldaron's Device Interface / GPU / OpenGL
// Copyright (c) 2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>
// Licensed under the MIT LICENSE

//! OpenGL implementation for adi_gpu.

extern crate ami;
extern crate awi;
extern crate afi;
extern crate asi_opengl;
extern crate adi_gpu_base;

use std::mem;

pub use base::Shape;
pub use base::Gradient;
pub use base::Model;
pub use base::TexCoords;

use ami::*;
use adi_gpu_base as base;
use asi_opengl::{ OpenGL, OpenGLBuilder };
use awi::WindowConnection;
use adi_gpu_base::ShapeHandle;

const SHADER_SOLID_FRAG: &'static [u8] = include_bytes!("shaders/solid-frag.glsl");
const SHADER_SOLID_VERT: &'static [u8] = include_bytes!("shaders/solid-vert.glsl");
const SHADER_GRADIENT_FRAG: &'static [u8] = include_bytes!("shaders/gradient-frag.glsl");
const SHADER_GRADIENT_VERT: &'static [u8] = include_bytes!("shaders/gradient-vert.glsl");
const SHADER_TEX_FRAG: &'static [u8] = include_bytes!("shaders/texture-frag.glsl");
const SHADER_TEX_VERT: &'static [u8] = include_bytes!("shaders/texture-vert.glsl");
const SHADER_FADED_VERT: &'static [u8] = include_bytes!("shaders/faded-vert.glsl");
const SHADER_TINTED_FRAG: &'static [u8] = include_bytes!("shaders/tinted-frag.glsl");
const SHADER_COMPLEX_VERT: &'static [u8] = include_bytes!("shaders/complex-vert.glsl");
const SHADER_COMPLEX_FRAG: &'static [u8] = include_bytes!("shaders/complex-frag.glsl");

const STYLE_GRADIENT: usize = 0;
const STYLE_TEXTURE: usize = 1;
const STYLE_FADED: usize = 2;
const STYLE_TINTED: usize = 3;
const STYLE_SOLID: usize = 4;
const STYLE_COMPLEX: usize = 5;

struct Style {
	shader: u32,
	matrix_uniform: i32,
	has_camera: i32,
	camera_uniform: i32,
	has_fog: i32,
	fog: i32,
	range: i32,
//	texture: i32,
	alpha: i32,
	color: i32,
	position: asi_opengl::Attribute,
	texpos: asi_opengl::Attribute,
	acolor: asi_opengl::Attribute,
}

impl Style {
	// Create a new style.
	fn new(context: &OpenGL, vert: &[u8], frag: &[u8], t: bool, a: bool,
		c: bool, g: bool) -> Style
	{
		let shader = context.shader(vert, frag);
		let matrix_uniform = context.uniform(shader, b"models_tfm\0");
		let has_camera = context.uniform(shader, b"has_camera\0");
		let camera_uniform = context.uniform(shader, b"matrix\0");
		let has_fog = context.uniform(shader, b"has_fog\0");
		let fog = context.uniform(shader, b"fog\0");
		let range = context.uniform(shader, b"range\0");
//		let texture = if t { context.uniform(shader, b"texture\0") }
//			else { -1 };
		let alpha = if a { context.uniform(shader, b"alpha\0") }
			else { -1 };
		let color = if c { context.uniform(shader, b"color\0") }
			else { -1 };
		let position = context.attribute(shader, b"position\0");
		let texpos = if t {
			context.attribute(shader, b"texpos\0")
		} else {
			asi_opengl::Attribute(-1)
		};
		let acolor = if g {
			context.attribute(shader, b"acolor\0")
		} else {
			asi_opengl::Attribute(-1)
		};

		Style {
			shader, matrix_uniform, has_camera, camera_uniform, fog,
			range, position, texpos, alpha, has_fog, color, acolor,
		}
	}
}

#[derive(Clone)]
struct ShapeData {
	style: usize,
	index_buffer: u32,
	index_count: u32,
	num_buffers: usize,
	buffers: [u32; 2],
	has_fog: bool,
	alpha: Option<f32>,
	color: Option<[f32; 4]>,
	transform: ami::Mat4, // Transformation matrix.
	texture: Option<asi_opengl::Texture>,
	vertex_buffer: u32,
	vertice_count: u32,
	bounds: [(f32, f32); 3], // xMinMax, yMinMax, zMinMax
	center: ::ami::Vec3<f32>,
	position: ::ami::Vec3<f32>,
}

struct ModelData {
	index_buffer: u32,
	index_count: u32,
	vertex_buffer: u32,
	// TODO alot could be in base as duplicate
	vertex_count: u32,
	bounds: [(f32, f32); 3], // xMinMax, yMinMax, zMinMax
	center: ::ami::Vec3<f32>,
}

struct TexcoordsData {
	vertex_buffer: u32,
	vertex_count: u32,
}

struct GradientData {
	vertex_buffer: u32,
	vertex_count: u32,
}

impl ::ami::Pos for ShapeData {
	fn posf(&self) -> ::ami::Vec3<f32> {
		self.position
	}

	fn posi(&self) -> ::ami::Vec3<i32> {
		self.position.into()
	}
}

/// To render anything with adi_gpu, you have to make a `Display`
pub struct Display {
	window: awi::Window,
	context: OpenGL,
	color: (f32, f32, f32),
	opaque_octree: ::ami::Octree<ShapeData>,
	alpha_octree: ::ami::Octree<ShapeData>,
	gui_vec: Vec<ShapeData>,
	models: Vec<ModelData>,
	texcoords: Vec<TexcoordsData>,
	gradients: Vec<GradientData>,
	opaque_sorted: Vec<u32>,
	alpha_sorted: Vec<u32>,
	styles: [Style; 6],
//	default_tc: u32,
//	upsidedown_tc: u32,
	xyz: (f32,f32,f32),
	rotate_xyz: (f32,f32,f32),
	frustum: ::ami::Frustum,
	ar: f32,
	projection: ::ami::Mat4,
}

impl base::Display for Display {
	type Texture = Texture;

	fn new(title: &str, icon: &afi::Graphic) -> Option<Self> {
		if let Some(tuple) = OpenGLBuilder::new() {
			let (builder, v) = tuple;
			let window = awi::Window::new(title, &icon, Some(v));

			let context = builder.to_opengl(match window.get_connection() {
				WindowConnection::Xcb(_, window) => // |
				//	WindowConnection::Windows(_, window) =>
				{
					unsafe {mem::transmute(window as usize)}
				},
				WindowConnection::Wayland => return None,
				WindowConnection::DirectFB => return None,
				WindowConnection::Android => return None,
				WindowConnection::IOS => return None,
				WindowConnection::AldaronsOS => return None,
				WindowConnection::Arduino => return None,
				WindowConnection::Switch => return None,
				WindowConnection::Web => return None,
				WindowConnection::NoOS => return None,
				_ => return None // TODO
			});

			// Set the settings.
			context.disable(0x0BD0 /*DITHER*/);
			context.enable(0x0B44/*CULL_FACE*/);
			context.enable(0x0BE2/*BLEND*/);
			context.blend();

			// Load shaders
			let style_solid = Style::new(&context,
				SHADER_SOLID_VERT, SHADER_SOLID_FRAG,
				false/*texture*/,false/*alpha*/,true/*color*/,
				false/*gradient*/);
			let style_gradient = Style::new(&context,
				SHADER_GRADIENT_VERT, SHADER_GRADIENT_FRAG,
				false/*texture*/,false/*alpha*/,false/*color*/,
				true/*gradient*/);
			let style_texture = Style::new(&context,
				SHADER_TEX_VERT, SHADER_TEX_FRAG,
				true/*texture*/,false/*alpha*/,false/*color*/,
				false/*gradient*/);
			let style_faded = Style::new(&context,
				SHADER_FADED_VERT, SHADER_TEX_FRAG,
				true/*texture*/,true/*alpha*/,false/*color*/,
				false/*gradient*/);
			let style_tinted = Style::new(&context,
				SHADER_TEX_VERT, SHADER_TINTED_FRAG,
				true/*texture*/,false/*alpha*/,true/*color*/,
				false/*gradient*/);
			let style_complex = Style::new(&context,
				SHADER_COMPLEX_VERT, SHADER_COMPLEX_FRAG,
				true/*texture*/,false/*alpha*/,false/*color*/,
				true/*gradient*/);

			// Generate buffers
			/* let tcs = context.new_buffers(2);

			let default_tc = tcs[0];
			let upsidedown_tc = tcs[1];

			context.bind_buffer(false, default_tc);
			context.set_buffer(false, &[
				0.0, 1.0,
				0.0, 0.0,
				1.0, 0.0,
				1.0, 1.0,
			]);

			context.bind_buffer(false, upsidedown_tc);
			context.set_buffer(false, &[
				0.0, 0.0,
				0.0, 1.0,
				1.0, 1.0,
				1.0, 0.0,
			]);*/

			let wh = window.wh();
			let ar = wh.0 as f32 / wh.1 as f32;

			let projection = base::projection(ar, 90.0);

			// Adjust the viewport
			context.viewport(wh.0, wh.1);

			let mut display = Display {
				window,
				context,
				color: (0.0, 0.0, 0.0),
				alpha_octree: ::ami::Octree::new(),
				opaque_octree: ::ami::Octree::new(),
				gui_vec: Vec::new(),
				opaque_sorted: Vec::new(),
				alpha_sorted: Vec::new(),
				models: Vec::new(),
				texcoords: Vec::new(),
				gradients: Vec::new(),
				styles: [
					style_gradient,
					style_texture,
					style_faded,
					style_tinted,
					style_solid,
					style_complex,
				],
				xyz: (0.0, 0.0, 0.0),
				rotate_xyz: (0.0, 0.0, 0.0),
				frustum: ::ami::Frustum::new(::ami::Vec3::new(0.0, 0.0, 0.0),
					100.0 /* TODO: Based on fog.0 + fog.1 */, 90.0,
					2.0 * ((45.0 * ::std::f32::consts::PI / 180.0).tan() / ar).atan(),
					0.0, 0.0
				), // TODO: COPIED FROM renderer/mod.rs
				ar,
				projection,
			};

			display.camera((0.0, 0.0, 0.0), (0.0, 0.0, 0.0));

			Some(display)
		} else {
			None
		}
	}

	fn color(&mut self, color: (f32, f32, f32)) {
		self.color = color;
		self.context.color(color.0, color.1, color.2);
	}

	fn update(&mut self) -> Option<awi::Input> {
		if let Some(input) = self.window.update() {
			return Some(input);
		}

		// Update Window:
		self.context.clear();

		// TODO: This is copied pretty much from adi_gpu_vulkan.  Move
		// to the base.

		let matrix = ::Mat4::new()
			.rotate(self.rotate_xyz.0, self.rotate_xyz.1,
				self.rotate_xyz.2)
			.translate(self.xyz.0, self.xyz.1, self.xyz.2);

		let frustum = matrix * self.frustum;

		// Opaque & Alpha Shapes need a camera.
		for i in (&self.styles).iter() {
			self.context.use_program(i.shader);
			self.context.set_int1(i.has_camera, 1);
		}

		// Enable for 3D depth testing
		self.context.enable(0x0B71/*GL_DEPTH_TEST*/);

		self.opaque_octree.nearest(&mut self.opaque_sorted, frustum);
		for id in self.opaque_sorted.iter() {
			let shape = &self.opaque_octree[*id];

			draw_shape(&self.context, &self.styles[shape.style],
				shape);
		}

		self.alpha_octree.farthest(&mut self.alpha_sorted, frustum);
		for id in self.alpha_sorted.iter() {
			let shape = &self.alpha_octree[*id];

			draw_shape(&self.context, &self.styles[shape.style],
				shape);
		}

		// Disable Depth Testing for GUI
		self.context.disable(0x0B71/*GL_DEPTH_TEST*/);

		// Gui Elements don't want a camera.
		for i in (&self.styles).iter() {
			self.context.use_program(i.shader);
			self.context.set_int1(i.has_camera, 0);
		}

		for shape in self.gui_vec.iter() {
			draw_shape(&self.context, &self.styles[shape.style],
				shape);
		}

		// end todo

		self.context.update();
		// Return None, there was no input, updated screen.
		None
	}

	fn camera(&mut self, xyz: (f32,f32,f32), rotate_xyz: (f32,f32,f32)) {
		// Set Camera
		self.xyz = xyz;
		self.rotate_xyz = rotate_xyz;

		// Write To Camera Uniforms.  TODO: only before use (not here).
		// TODO this assignment copied from vulkan implementation.  Put
		// in the base library.
		let cam = (::Mat4::new()
			.translate(-self.xyz.0, -self.xyz.1, -self.xyz.2)
			.rotate(-self.rotate_xyz.0, -self.rotate_xyz.1,
				-self.rotate_xyz.2) * self.projection).0;

		for i in (&self.styles).iter() {
			self.context.use_program(i.shader);
			self.context.set_mat4(i.camera_uniform, &cam);
		}
	}

	fn model(&mut self, vertices: &[f32], indices: &[u32]) -> Model {
		// TODO most is duplicate from other implementation.
		let index = self.models.len();

		let buffers = self.context.new_buffers(2);

		let index_buffer = buffers[0];
		self.context.bind_buffer(true, index_buffer);
		self.context.set_buffer(true, indices);

		let vertex_buffer = buffers[1];
		self.context.bind_buffer(false, vertex_buffer);
		self.context.set_buffer(false, vertices);

		let mut xtot = vertices[0];
		let mut ytot = vertices[1];
		let mut ztot = vertices[2];
		let mut xmin = vertices[0];
		let mut ymin = vertices[1];
		let mut zmin = vertices[2];
		let mut xmax = vertices[0];
		let mut ymax = vertices[1];
		let mut zmax = vertices[2];

		for i in 4..vertices.len() {
			match i % 4 {
				0 => {
					let x = vertices[i];
					xtot += x;
					if x < xmin {
						xmin = x;
					} else if x > xmax {
						xmax = x;
					}
				},
				1 => {
					let y = vertices[i];
					ytot += y;
					if y < ymin {
						ymin = y;
					} else if y > ymax {
						ymax = y;
					}
				},
				2 => {
					let z = vertices[i];
					ztot += z;
					if z < zmin {
						zmin = z;
					} else if z > zmax {
						zmax = z;
					}
				},
				_ => { },
			}
		}

		let n = (vertices.len() / 4) as f32;

		self.models.push(ModelData {
			index_buffer, index_count: indices.len() as u32,
			vertex_buffer,
			vertex_count: vertices.len() as u32 / 4,
			bounds: [(xmin, xmax), (ymin, ymax), (zmin, zmax)],
			center: ::ami::Vec3::new(xtot / n, ytot / n, ztot / n),
		});

		Model(index)
//		Model(self.renderer.model(vertices, indices))
	}

	fn fog(&mut self, fog: Option<(f32, f32)>) -> () {
		let fogc = [self.color.0, self.color.1, self.color.2, 1.0];
		let fogr = if let Some(fog) = fog {
			[fog.0, fog.1]
		} else {
			[::std::f32::MAX, 0.0]
		};

		for i in (&self.styles).iter() {
			self.context.use_program(i.shader);
			self.context.set_vec4(i.fog, &fogc);
			self.context.set_vec2(i.range, &fogr);
		}
	}

	fn texture(&mut self, graphic: afi::Graphic) -> Texture {
		let (w, h, pixels) = graphic.as_slice();

		let t = self.context.new_texture();

		self.context.use_texture(&t);
		self.context.set_texture(w, h, pixels);

		Texture { t, w, h }
	}

	fn gradient(&mut self, colors: &[f32]) -> Gradient {
		// TODO: A lot of duplication here from adi_gpu_vulkan.  Put in
		// base.
		let vertex_buffer = self.context.new_buffers(1)[0];

		self.context.bind_buffer(false, vertex_buffer);
		self.context.set_buffer(false, colors);

		let a = self.gradients.len();

		self.gradients.push(GradientData {
			vertex_buffer,
			vertex_count: colors.len() as u32 / 4,
		});

		Gradient(a)
	}

	fn texcoords(&mut self, texcoords: &[f32]) -> TexCoords {
		// TODO: A lot of duplication here from adi_gpu_vulkan.  Put in
		// base.
		let vertex_buffer = self.context.new_buffers(1)[0];

		self.context.bind_buffer(false, vertex_buffer);
		self.context.set_buffer(false, texcoords);

		let a = self.texcoords.len();

		self.texcoords.push(TexcoordsData {
			vertex_buffer,
			vertex_count: texcoords.len() as u32 / 4,
		});

		TexCoords(a)
	}

	fn set_texture(&mut self, texture: &mut Self::Texture, pixels: &[u32]) {
		self.context.use_texture(&texture.t);
		self.context.texture_update(texture.w, texture.h, pixels);
	}

	#[inline(always)]
	fn shape_solid(&mut self, model: &Model, transform: Mat4,
		color: [f32; 4], blending: bool, fog: bool, camera: bool)
		-> Shape
	{
		let shape = ShapeData {
			style: STYLE_SOLID,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 0,
			buffers: [
				unsafe { mem::uninitialized() },
				unsafe { mem::uninitialized() }
			],
			has_fog: fog,
			alpha: None,
			color: Some(color),
			texture: None,
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if blending {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		} else {
			base::ShapeHandle::Opaque(self.opaque_octree.add(shape))
		})
	}

	#[inline(always)]
	fn shape_gradient(&mut self, model: &Model, transform: Mat4,
		colors: Gradient, blending: bool, fog: bool, camera: bool)
		-> Shape
	{
		// TODO: is copied from adi_gpu_vulkan, move to base
		if self.models[model.0].vertex_count
			!= self.gradients[colors.0].vertex_count
		{
			panic!("TexCoord length doesn't match gradient length");
		}

		let shape = ShapeData {
			style: STYLE_GRADIENT,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 1,
			buffers: [
				self.gradients[colors.0].vertex_buffer,
				unsafe { mem::uninitialized() }
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: None,
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		println!("JUST SET: {}", shape.vertex_buffer);

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if blending {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		} else {
			base::ShapeHandle::Opaque(self.opaque_octree.add(shape))
		})
	}

	#[inline(always)]
	fn shape_texture(&mut self, model: &Model, transform: Mat4,
		texture: Texture, tc: TexCoords, blending: bool, fog: bool,
		camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let shape = ShapeData {
			style: STYLE_TEXTURE,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 1,
			buffers: [
				self.texcoords[tc.0].vertex_buffer,
				unsafe { mem::uninitialized() }
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: Some(texture.t),
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if blending {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		} else {
			base::ShapeHandle::Opaque(self.opaque_octree.add(shape))
		})
	}

	#[inline(always)]
	fn shape_faded(&mut self, model: &Model, transform: Mat4,
		texture: Texture, tc: TexCoords, alpha: f32, fog: bool,
		camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let shape = ShapeData {
			style: STYLE_FADED,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 1,
			buffers: [
				self.texcoords[tc.0].vertex_buffer,
				unsafe { mem::uninitialized() }
			],
			has_fog: fog,
			alpha: Some(alpha),
			color: None,
			texture: Some(texture.t),
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		})
	}

	#[inline(always)]
	fn shape_tinted(&mut self, model: &Model, transform: Mat4,
		texture: Texture, tc: TexCoords, tint: [f32; 4], blending: bool,
		fog: bool, camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let shape = ShapeData {
			style: STYLE_TINTED,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 1,
			buffers: [
				self.texcoords[tc.0].vertex_buffer,
				unsafe { mem::uninitialized() },
			],
			has_fog: fog,
			alpha: None,
			color: Some(tint),
			texture: Some(texture.t),
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if blending {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		} else {
			base::ShapeHandle::Opaque(self.opaque_octree.add(shape))
		})
	}

	#[inline(always)]
	fn shape_complex(&mut self, model: &Model, transform: Mat4,
		texture: Texture, tc: TexCoords, tints: Gradient,
		blending: bool, fog: bool, camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		// TODO: is copied from adi_gpu_vulkan, move to base
		if self.models[model.0].vertex_count
			!= self.gradients[tints.0].vertex_count
		{
			panic!("TexCoord length doesn't match gradient length");
		}

		let shape = ShapeData {
			style: STYLE_COMPLEX,
			index_buffer: self.models[model.0].index_buffer,
			index_count: self.models[model.0].index_count,
			num_buffers: 2,
			buffers: [
				self.texcoords[tc.0].vertex_buffer,
				self.gradients[tints.0].vertex_buffer,
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: Some(texture.t),
			vertex_buffer: self.models[model.0].vertex_buffer,
			vertice_count: self.models[model.0].vertex_count,
			bounds: self.models[model.0].bounds,
			center: self.models[model.0].center,
			position: transform * self.models[model.0].center,
			transform, // Transformation matrix.
		};

		base::new_shape(if !camera && !fog {
			self.gui_vec.push(shape);
			base::ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if blending {
			base::ShapeHandle::Alpha(self.alpha_octree.add(shape))
		} else {
			base::ShapeHandle::Opaque(self.opaque_octree.add(shape))
		})
	}

	fn transform(&mut self, shape: &mut Shape, transform: Mat4) {
		// TODO: put in base, some is copy from vulkan implementation.
		match base::get_shape(shape) {
			ShapeHandle::Opaque(ref mut x) => {
				let mut shape = self.opaque_octree[*x].clone();

				shape.position = transform *
					self.opaque_octree[*x].center;
				self.opaque_octree.modify(x, shape);

				self.opaque_octree[*x].transform = transform;
			},
			ShapeHandle::Alpha(ref mut x) => {
				let mut shape = self.alpha_octree[*x].clone();

				shape.position = transform *
					self.alpha_octree[*x].center;
				self.alpha_octree.modify(x, shape);

				self.alpha_octree[*x].transform = transform;
			},
			ShapeHandle::Gui(x) => {
				let x = x as usize; // for indexing
				let mut shape = self.gui_vec[x].clone();

				shape.position = transform *
					self.gui_vec[x].center;

				self.gui_vec[x].transform = transform;
			},
		}
	}

	fn resize(&mut self, wh: (u32, u32)) -> () {
		let xyz = self.xyz;
		let rotate_xyz = self.rotate_xyz;

		self.ar = wh.0 as f32 / wh.1 as f32;
		self.frustum = ::ami::Frustum::new(
			self.frustum.center,
			self.frustum.radius,
			90.0, 2.0 * ((45.0 * ::std::f32::consts::PI / 180.0)
				.tan() / self.ar).atan(),
			self.frustum.xrot, self.frustum.yrot);

		self.context.viewport(wh.0, wh.1);

		self.projection = ::base::projection(self.ar, 90.0);
		self.camera(xyz, rotate_xyz);
	}

	fn wh(&self) -> (u32, u32) {
		self.window.wh()
	}
}

#[derive(Copy, Clone)]
pub struct Texture {
	t: asi_opengl::Texture,
	w: u32,
	h: u32,
}

impl base::Texture for Texture {
	/// Get the width and height.
	fn wh(&self) -> (u32, u32) {
		(self.w, self.h)
	}
}

fn draw_shape(context: &OpenGL, style: &Style, shape: &ShapeData) {
	context.use_program(style.shader);
	context.set_mat4(style.matrix_uniform, &shape.transform.0);

	if style.texpos.0 != -1 {
		// Bind texture coordinates buffer
		context.bind_buffer(false, shape.buffers[0]);
		// Bind vertex buffer to attribute
		context.vertex_attrib(&style.texpos);
		// Bind the texture
		context.use_texture(&shape.texture.unwrap());
	}

	if style.acolor.0 != -1 {
		// Bind color buffer
		context.bind_buffer(false, shape.buffers[0]);
		// Bind vertex buffer to attribute
		context.vertex_attrib(&style.acolor);
	}

	if style.alpha != -1 {
		context.set_vec1(style.alpha, shape.alpha.unwrap());
	}

	if style.color != -1 {
		context.set_vec4(style.color, &shape.color.unwrap());
	}

	if shape.has_fog {
		context.set_int1(style.has_fog, 1);
	} else {
		context.set_int1(style.has_fog, 0);
	}

	context.bind_buffer(true, shape.index_buffer);
	context.bind_buffer(false, shape.vertex_buffer);
	context.vertex_attrib(&style.position);
	context.draw_elements(shape.index_count);
}
