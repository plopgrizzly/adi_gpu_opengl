// "adi_gpu_opengl" crate - Licensed under the MIT LICENSE
//  * Copyright (c) 2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>
//
//! OpenGL implementation for adi_gpu.

extern crate ami;
extern crate asi_opengl;
extern crate adi_gpu_base;

use std::mem;

pub use base::Shape;
pub use base::Gradient;
pub use base::Model;
pub use base::TexCoords;
pub use base::Texture;

use ami::*;
use adi_gpu_base as base;
use asi_opengl::{
	OpenGL, OpenGLBuilder, VertexData, Program, Buffer, UniformData,
	Feature, Topology,
};
use adi_gpu_base::{ Graphic, WindowConnection, ShapeHandle };

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
	shader: Program,
	matrix_uniform: UniformData,
	has_camera: UniformData,
	camera_uniform: UniformData,
	has_fog: UniformData,
	fog: UniformData,
	range: UniformData,
	alpha: UniformData,
	color: UniformData,
	position: VertexData,
	texpos: VertexData,
	acolor: VertexData,
}

impl Style {
	// Create a new style.
	fn new(context: &OpenGL, vert: &[u8], frag: &[u8]) -> Style {
		let shader = Program::new(context, vert, frag);
		let matrix_uniform = shader.uniform(b"models_tfm\0");
		let has_camera = shader.uniform(b"has_camera\0");
		let camera_uniform = shader.uniform(b"matrix\0");
		let has_fog = shader.uniform(b"has_fog\0");
		let fog = shader.uniform(b"fog\0");
		let range = shader.uniform(b"range\0");
		let alpha = shader.uniform(b"alpha\0");
		let color = shader.uniform(b"color\0");
		let position = shader.vertex_data(b"position\0");
		let texpos = shader.vertex_data(b"texpos\0");
		let acolor = shader.vertex_data(b"acolor\0");

		Style {
			shader, matrix_uniform, has_camera, camera_uniform, fog,
			range, position, texpos, alpha, has_fog, color, acolor,
		}
	}
}

struct ShapeData {
	style: usize,
	buffers: [Option<Buffer>; 2],
	has_fog: bool,
	alpha: Option<f32>,
	color: Option<[f32; 4]>,
	transform: ami::Mat4, // Transformation matrix.
	texture: Option<asi_opengl::Texture>,
	vertex_buffer: Buffer,
	bbox: BBox,
	model: usize,
	fans: Vec<(u32, u32)>,
}

struct ModelData {
	vertex_buffer: Buffer,
	// TODO alot could be in base as duplicate
	vertex_count: u32,
	points: Vec<f32>,
	fans: Vec<(u32, u32)>,
}

struct TexcoordsData {
	vertex_buffer: Buffer,
	vertex_count: u32,
}

struct GradientData {
	vertex_buffer: Buffer,
	vertex_count: u32,
}

struct TextureData {
	t: asi_opengl::Texture,
	w: u32,
	h: u32,
}

impl ::ami::Collider for ShapeData {
	fn bbox(&self) -> ::ami::BBox {
		self.bbox
	}
}

/// To render anything with adi_gpu, you have to make a `Display`
pub struct Display {
	window: adi_gpu_base::Window,
	context: OpenGL,
	color: (f32, f32, f32),
	opaque_octree: ::ami::Octree<ShapeData>,
	alpha_octree: ::ami::Octree<ShapeData>,
	gui_vec: Vec<ShapeData>,
	models: Vec<ModelData>,
	texcoords: Vec<TexcoordsData>,
	gradients: Vec<GradientData>,
	textures: Vec<TextureData>,
	opaque_sorted: Vec<u32>,
	alpha_sorted: Vec<u32>,
	styles: [Style; 6],
	xyz: (f32,f32,f32),
	rotate_xyz: (f32,f32,f32),
	frustum: ::ami::Frustum,
	ar: f64,
	projection: ::ami::Mat4,
}

pub fn new<G: AsRef<Graphic>>(title: &str, icon: G)
	-> Result<Box<Display>, &'static str>
{
	if let Some(tuple) = OpenGLBuilder::new() {
		let (builder, v) = tuple;
		let window = adi_gpu_base::Window::new(title,
			icon.as_ref(), Some(v));

		let context = builder.to_opengl(match window.get_connection() {
			WindowConnection::Xcb(_, window) => // |
			//	WindowConnection::Windows(_, window) =>
			{
				unsafe {mem::transmute(window as usize)}
			},
			WindowConnection::Windows(_, window) => {
				window
			}
			WindowConnection::Wayland => return Err(
				"OpenGL support on Wayland is WIP"),
			WindowConnection::DirectFB => return Err(
				"OpenGL support on DirectFB is WIP"),
			WindowConnection::Android => return Err(
				"OpenGL support on Android is WIP"),
			WindowConnection::IOS => return Err(
				"OpenGL support on iOS is WIP"),
			WindowConnection::AldaronsOS => return Err(
				"AldaronsOS doesn't support OpenGL"),
			WindowConnection::Arduino => return Err(
				"Arduino doesn't support OpenGL"),
			WindowConnection::Switch => return Err(
				"Nintendo Switch doesn't support OpenGL"),
			WindowConnection::Web => return Err(
				"WebGL support is WIP"),
			WindowConnection::NoOS => return Err(
				"NoOS doesn't support OpenGL"),
		});

		// Set the settings.
		context.disable(Feature::Dither);
		context.enable(Feature::CullFace);
		context.enable(Feature::Blend);
		context.blend();

		// Load shaders
		let style_solid = Style::new(&context,
			SHADER_SOLID_VERT, SHADER_SOLID_FRAG);
		let style_gradient = Style::new(&context,
			SHADER_GRADIENT_VERT, SHADER_GRADIENT_FRAG);
		let style_texture = Style::new(&context,
			SHADER_TEX_VERT, SHADER_TEX_FRAG);
		let style_faded = Style::new(&context,
			SHADER_FADED_VERT, SHADER_TEX_FRAG);
		let style_tinted = Style::new(&context,
			SHADER_TEX_VERT, SHADER_TINTED_FRAG);
		let style_complex = Style::new(&context,
			SHADER_COMPLEX_VERT, SHADER_COMPLEX_FRAG);

		let wh = window.wh();
		let ar = wh.0 as f64 / wh.1 as f64;

		let projection = base::projection(ar, 90.0);

		// Adjust the viewport
		context.viewport(wh.0, wh.1);

		let mut display = ::Display {
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
			textures: Vec::new(),
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
				(2.0 * ((45.0 * ::std::f64::consts::PI / 180.0).tan() / ar).atan()) as f32,
				0.0, 0.0
			), // TODO: COPIED FROM renderer/mod.rs
			ar,
			projection,
		};

		use base::Display;
		display.camera((0.0, 0.0, 0.0), (0.0, 0.0, 0.0));

		Ok(Box::new(display))
	} else {
		Err("Couldn't find OpenGL!")
	}
}

impl base::Display for Display {
	fn color(&mut self, color: (f32, f32, f32)) {
		self.color = color;
		self.context.color(color.0, color.1, color.2);
	}

	fn update(&mut self) -> Option<adi_gpu_base::Input> {
		if let Some(input) = self.window.update() {
			return Some(input);
		}

		// Update Window:
		// TODO: This is copied pretty much from adi_gpu_vulkan.  Move
		// to the base.
		let matrix = IDENTITY
			.rotate(self.rotate_xyz.0, self.rotate_xyz.1,
				self.rotate_xyz.2)
			.translate(self.xyz.0, self.xyz.1, self.xyz.2);

		let frustum = matrix * self.frustum;

		// Opaque & Alpha Shapes need a camera.
		for i in (&self.styles).iter() {
			i.has_camera.set_int1(1);
		}

		// Enable for 3D depth testing
		self.context.enable(Feature::DepthTest);

		self.opaque_octree.nearest(&mut self.opaque_sorted, frustum);
		for id in self.opaque_sorted.iter() {
			let shape = &self.opaque_octree[*id];

			draw_shape(&self.styles[shape.style], shape);
		}

		self.alpha_octree.farthest(&mut self.alpha_sorted, frustum);
		for id in self.alpha_sorted.iter() {
			let shape = &self.alpha_octree[*id];

			draw_shape(&self.styles[shape.style], shape);
		}

		// Disable Depth Testing for GUI
		self.context.disable(Feature::DepthTest);

		// Gui Elements don't want a camera.
		for i in (&self.styles).iter() {
			i.has_camera.set_int1(0);
		}

		for shape in self.gui_vec.iter() {
			draw_shape(&self.styles[shape.style], shape);
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
		let cam = (IDENTITY
			.translate(-self.xyz.0, -self.xyz.1, -self.xyz.2)
			.rotate(-self.rotate_xyz.0, -self.rotate_xyz.1,
				-self.rotate_xyz.2) * self.projection)
			.to_f32_array();

		for i in (&self.styles).iter() {
			i.camera_uniform.set_mat4(&cam);
		}
	}

	fn model(&mut self, vertices: &[f32], fans: Vec<(u32, u32)>) -> Model {
		// TODO most is duplicate from other implementation.
		let index = self.models.len();

		let buffer = Buffer::new(&self.context);

		let vertex_buffer = buffer;
		vertex_buffer.set(vertices);

		let points = vertices.to_vec();

		self.models.push(ModelData {
			vertex_buffer, vertex_count: vertices.len() as u32 / 4,
			points, fans
		});

		Model(index)
	}

	fn fog(&mut self, fog: Option<(f32, f32)>) -> () {
		let fogc = [self.color.0, self.color.1, self.color.2, 1.0];
		let fogr = if let Some(fog) = fog {
			[fog.0, fog.1]
		} else {
			[::std::f32::MAX, 0.0]
		};

		for i in (&self.styles).iter() {
			i.fog.set_vec4(&fogc);
			i.range.set_vec2(&fogr);
		}
	}

	fn texture(&mut self, graphic: &Graphic) -> Texture {
		let (w, h, pixels) = graphic.as_ref().as_slice();

		let t = self.context.texture();

		t.set(w, h, pixels);

		let a = self.textures.len();

		self.textures.push(TextureData { t, w, h });

		Texture(a)
	}

	fn gradient(&mut self, colors: &[f32]) -> Gradient {
		// TODO: A lot of duplication here from adi_gpu_vulkan.  Put in
		// base.
		let vertex_buffer = Buffer::new(&self.context);
		vertex_buffer.set(colors);

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
		let vertex_buffer = Buffer::new(&self.context);
		vertex_buffer.set(texcoords);

		let a = self.texcoords.len();

		self.texcoords.push(TexcoordsData {
			vertex_buffer,
			vertex_count: texcoords.len() as u32 / 4,
		});

		TexCoords(a)
	}

	fn set_texture(&mut self, texture: &mut Texture, pixels: &[u32]) {
		self.textures[texture.0].t.update(self.textures[texture.0].w,
			self.textures[texture.0].h, pixels);
	}

	#[inline(always)]
	fn shape_solid(&mut self, model: &Model, transform: Mat4,
		color: [f32; 4], blending: bool, fog: bool, camera: bool)
		-> Shape
	{
		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_SOLID,
			buffers: [None, None],
			has_fog: fog,
			alpha: None,
			color: Some(color),
			texture: None,
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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

		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_GRADIENT,
			buffers: [
				Some(self.gradients[colors.0].vertex_buffer.clone()),
				None
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: None,
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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
	fn shape_texture(&mut self, model: &Model, transform: Mat4,
		texture: &Texture, tc: TexCoords, blending: bool, fog: bool,
		camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_TEXTURE,
			buffers: [
				Some(self.texcoords[tc.0].vertex_buffer.clone()),
				None
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: Some(self.textures[texture.0].t.clone()),
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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
		texture: &Texture, tc: TexCoords, alpha: f32, fog: bool,
		camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_FADED,
			buffers: [
				Some(self.texcoords[tc.0].vertex_buffer.clone()),
				None
			],
			has_fog: fog,
			alpha: Some(alpha),
			color: None,
			texture: Some(self.textures[texture.0].t.clone()),
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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
		texture: &Texture, tc: TexCoords, tint: [f32; 4], blending: bool,
		fog: bool, camera: bool) -> Shape
	{
		// TODO: from adi_gpu_vulkan, move to the base
		if self.models[model.0].vertex_count
			!= self.texcoords[tc.0].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_TINTED,
			buffers: [
				Some(self.texcoords[tc.0].vertex_buffer.clone()),
				None,
			],
			has_fog: fog,
			alpha: None,
			color: Some(tint),
			texture: Some(self.textures[texture.0].t.clone()),
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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
		texture: &Texture, tc: TexCoords, tints: Gradient,
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

		let bbox = base::vertices_to_bbox(
			self.models[model.0].points.as_slice(), transform
		);

		let shape = ShapeData {
			style: STYLE_COMPLEX,
			buffers: [
				Some(self.texcoords[tc.0].vertex_buffer.clone()),
				Some(self.gradients[tints.0].vertex_buffer.clone()),
			],
			has_fog: fog,
			alpha: None,
			color: None,
			texture: Some(self.textures[texture.0].t.clone()),
			vertex_buffer: self.models[model.0].vertex_buffer.clone(),
			bbox,
			model: model.0,
			transform, // Transformation matrix.
			fans: self.models[model.0].fans.clone(),
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
				let model = self.opaque_octree[*x].model;
				let bbox = base::vertices_to_bbox(
					self.models[model].points.as_slice(),
					transform);

				self.opaque_octree[*x].bbox = bbox;
				let shape = self.opaque_octree.remove(*x);
				*x = self.opaque_octree.add(shape);

				self.opaque_octree[*x].transform = transform;
			},
			ShapeHandle::Alpha(ref mut x) => {
				let model = self.alpha_octree[*x].model;
				let bbox = base::vertices_to_bbox(
					self.models[model].points.as_slice(),
					transform);

				self.alpha_octree[*x].bbox = bbox;
				let shape = self.alpha_octree.remove(*x);
				*x = self.alpha_octree.add(shape);

				self.alpha_octree[*x].transform = transform;
			},
			ShapeHandle::Gui(x) => {
				let x = x as usize; // for indexing

				let model = self.gui_vec[x].model;
				let bbox = base::vertices_to_bbox(
					self.models[model].points.as_slice(),
					transform);

				self.gui_vec[x].bbox = bbox;

				self.gui_vec[x].transform = transform;
			},
		}
	}

	fn collision(&self, shape: &Shape, force: &mut Vec3) -> Option<u32> {
		match base::get_shape(shape) {
			ShapeHandle::Opaque(x) => {
				self.opaque_octree.bounce_test(x, force)
			},
			ShapeHandle::Alpha(x) => {
				self.alpha_octree.bounce_test(x, force)
			},
			_ => panic!("No gui collision detection!")
		}
	}

	fn resize(&mut self, wh: (u32, u32)) -> () {
		let xyz = self.xyz;
		let rotate_xyz = self.rotate_xyz;

		self.ar = wh.0 as f64 / wh.1 as f64;
		self.frustum = ::ami::Frustum::new(
			self.frustum.center,
			self.frustum.radius,
			90.0, (2.0 * ((45.0 * ::std::f64::consts::PI / 180.0)
				.tan() / self.ar).atan()) as f32,
			self.frustum.xrot, self.frustum.yrot);

		self.context.viewport(wh.0, wh.1);

		self.projection = ::base::projection(self.ar, 90.0);
		self.camera(xyz, rotate_xyz);
	}

	fn wh(&self) -> (u32, u32) {
		self.window.wh()
	}
}

fn draw_shape(style: &Style, shape: &ShapeData) {
	style.matrix_uniform.set_mat4(&shape.transform.to_f32_array());

	if !style.texpos.is_none() {
		// Set texpos for the program from the texpos buffer.
		style.texpos.set(shape.buffers[0].as_ref().unwrap());
		// Bind the texture
		shape.texture.as_ref().unwrap().bind();
	}

	if !style.acolor.is_none() {
		// Set colors for the program from the color buffer.
		// TODO: probably shouldn't be same buffer as texpos.
		style.acolor.set(shape.buffers[0].as_ref().unwrap());
	}

	if !style.alpha.is_none() {
		style.alpha.set_vec1(shape.alpha.unwrap());
	}

	if !style.color.is_none() {
		style.color.set_vec4(&shape.color.unwrap());
	}

	if shape.has_fog {
		style.has_fog.set_int1(1);
	} else {
		style.has_fog.set_int1(0);
	}

	// Set vertices for the program from the vertex buffer.
	style.position.set(&shape.vertex_buffer);
	for i in shape.fans.iter() {
		style.shader.draw_arrays(Topology::TriangleFan, i.0..i.1);
	}
}
