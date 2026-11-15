use std::fmt::Write;
use tgprep_model::doc::{Adjustment, Crop, Edits};
use tgprep_model::geometry::{Size, output_to_source_uv};
use wasm_bindgen::prelude::*;
use web_sys::{
    HtmlCanvasElement, ImageBitmap, OffscreenCanvas, WebGl2RenderingContext as Gl, WebGlProgram,
    WebGlShader, WebGlUniformLocation,
};

const VERTEX_SHADER: &str = "#version 300 es
in vec2 position;
out vec2 outputUv;
void main() {
  outputUv = vec2(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
  gl_Position = vec4(position, 0.0, 1.0);
}";

const FRAGMENT_BODY: &str = "
float luma(vec3 rgb) { return dot(rgb, vec3(0.2126, 0.7152, 0.0722)); }
float hash(vec2 point) { return fract(sin(dot(point, vec2(12.9898, 78.233))) * 43758.5453); }
vec3 at(vec2 uv) { return texture(image, uv).rgb; }

void main() {
  vec2 uv = (outputToSource * vec3(outputUv, 1.0)).xy;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) { color = background; return; }
  vec3 rgb = at(uv);

  if (SHARPNESS > 0.0) {
    vec2 dx = (outputToSource * vec3(1.0 / outputSize.x, 0.0, 0.0)).xy;
    vec2 dy = (outputToSource * vec3(0.0, 1.0 / outputSize.y, 0.0)).xy;
    vec3 blur = (at(uv + dx) + at(uv - dx) + at(uv + dy) + at(uv - dy)) * 0.15
      + (at(uv + dx + dy) + at(uv - dx - dy) + at(uv + dx - dy) + at(uv - dx + dy)) * 0.1;
    rgb += (rgb - blur) * SHARPNESS * 2.5;
  }

  vec3 linear = pow(max(rgb, 0.0), vec3(2.2));
  linear *= exp2(EXPOSURE * 2.0);
  linear *= vec3(1.0 + TEMPERATURE * 0.25, 1.0 - TINT * 0.18, 1.0 - TEMPERATURE * 0.25);
  rgb = pow(max(linear, 0.0), vec3(1.0 / 2.2));

  float lightness = luma(rgb);
  rgb += SHADOWS * 0.3 * (1.0 - smoothstep(0.0, 0.55, lightness));
  rgb += HIGHLIGHTS * 0.3 * smoothstep(0.45, 1.0, lightness);
  rgb = pow(max(rgb, 0.0), vec3(exp2(-BRIGHTNESS * 0.8)));
  rgb = (rgb - 0.5) * exp2(CONTRAST) + 0.5;

  lightness = luma(rgb);
  float chroma = max(rgb.r, max(rgb.g, rgb.b)) - min(rgb.r, min(rgb.g, rgb.b));
  rgb = mix(vec3(lightness), rgb, 1.0 + SATURATION + VIBRANCE * (1.0 - clamp(chroma, 0.0, 1.0)));

  rgb = mix(rgb, rgb * 0.82 + 0.12, FADE);

  vec2 centered = (outputUv - 0.5) * 2.0 * normalize(outputSize) * 1.4142;
  float edge = smoothstep(0.35, 1.25, length(centered));
  rgb = VIGNETTE < 0.0 ? rgb * (1.0 + VIGNETTE * edge * 0.9) : mix(rgb, vec3(1.0), VIGNETTE * edge * 0.6);

  rgb += (hash(floor(outputUv * outputSize)) - 0.5) * GRAIN * 0.2;
  color = vec4(clamp(rgb, 0.0, 1.0), 1.0);
}";

fn fragment_shader() -> String {
    let mut source = format!(
        "#version 300 es\nprecision highp float;\nin vec2 outputUv;\nout vec4 color;\nuniform sampler2D image;\nuniform mat3 outputToSource;\nuniform vec2 outputSize;\nuniform vec4 background;\nuniform float adjust[{}];\n",
        Adjustment::ALL.len()
    );
    for (index, adjustment) in Adjustment::ALL.iter().enumerate() {
        let _ = writeln!(
            source,
            "#define {} adjust[{index}]",
            adjustment.key().to_uppercase()
        );
    }
    source.push_str(FRAGMENT_BODY);
    source
}

#[derive(Clone)]
pub enum Canvas {
    Element(HtmlCanvasElement),
    Offscreen(OffscreenCanvas),
}

impl Canvas {
    fn context(&self) -> Result<Option<js_sys::Object>, JsValue> {
        let options = js_sys::Object::new();
        for (name, value) in [
            ("premultipliedAlpha", false),
            ("preserveDrawingBuffer", true),
            ("antialias", false),
        ] {
            let _ = js_sys::Reflect::set(
                &options,
                &JsValue::from_str(name),
                &JsValue::from_bool(value),
            );
        }
        match self {
            Self::Element(canvas) => canvas.get_context_with_context_options("webgl2", &options),
            Self::Offscreen(canvas) => canvas.get_context_with_context_options("webgl2", &options),
        }
    }

    fn resize(&self, width: u32, height: u32) {
        match self {
            Self::Element(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
            Self::Offscreen(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }
    }
}

struct Locations {
    output_to_source: WebGlUniformLocation,
    output_size: WebGlUniformLocation,
    background: WebGlUniformLocation,
    adjust: WebGlUniformLocation,
}

pub struct Renderer {
    gl: Gl,
    locations: Locations,
    uploaded: Option<ImageBitmap>,
    pub max_texture_size: f64,
    pub canvas: Canvas,
}

fn compile(gl: &Gl, kind: u32, code: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("shader"))?;
    gl.shader_source(&shader, code);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        == Some(true)
    {
        Ok(shader)
    } else {
        Err(JsValue::from_str(
            &gl.get_shader_info_log(&shader)
                .unwrap_or_else(|| "shader".into()),
        ))
    }
}

fn locate(gl: &Gl, program: &WebGlProgram, name: &str) -> Result<WebGlUniformLocation, JsValue> {
    gl.get_uniform_location(program, name)
        .ok_or_else(|| JsValue::from_str(name))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "WebGL takes single precision"
)]
fn single(value: f64) -> f32 {
    value as f32
}

impl Renderer {
    pub fn new(canvas: Canvas) -> Result<Self, JsValue> {
        let gl: Gl = canvas
            .context()?
            .ok_or_else(|| JsValue::from_str("WebGL2 is not available"))?
            .dyn_into()?;
        let program = gl
            .create_program()
            .ok_or_else(|| JsValue::from_str("program"))?;
        gl.attach_shader(&program, &compile(&gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?);
        gl.attach_shader(
            &program,
            &compile(&gl, Gl::FRAGMENT_SHADER, &fragment_shader())?,
        );
        gl.link_program(&program);
        gl.use_program(Some(&program));

        gl.bind_buffer(Gl::ARRAY_BUFFER, gl.create_buffer().as_ref());
        gl.buffer_data_with_array_buffer_view(
            Gl::ARRAY_BUFFER,
            &js_sys::Float32Array::from(&[-1.0_f32, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0][..]),
            Gl::STATIC_DRAW,
        );
        let position = u32::try_from(gl.get_attrib_location(&program, "position"))
            .map_err(|_| JsValue::from_str("position"))?;
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_with_i32(position, 2, Gl::FLOAT, false, 0, 0);

        gl.bind_texture(Gl::TEXTURE_2D, gl.create_texture().as_ref());
        #[expect(clippy::cast_possible_wrap, reason = "WebGL enums fit in i32")]
        for (parameter, value) in [
            (Gl::TEXTURE_MIN_FILTER, Gl::LINEAR_MIPMAP_LINEAR as i32),
            (Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32),
            (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32),
            (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32),
        ] {
            gl.tex_parameteri(Gl::TEXTURE_2D, parameter, value);
        }
        let locations = Locations {
            output_to_source: locate(&gl, &program, "outputToSource")?,
            output_size: locate(&gl, &program, "outputSize")?,
            background: locate(&gl, &program, "background")?,
            adjust: locate(&gl, &program, "adjust")?,
        };
        let max_texture_size = gl
            .get_parameter(Gl::MAX_TEXTURE_SIZE)?
            .as_f64()
            .unwrap_or(4096.0);
        Ok(Self {
            gl,
            locations,
            uploaded: None,
            max_texture_size,
            canvas,
        })
    }

    pub fn render(
        &mut self,
        source: &ImageBitmap,
        edits: &Edits,
        crop: Crop,
        output: (u32, u32),
    ) -> Result<(), JsValue> {
        let gl = &self.gl;
        if self.uploaded.as_ref() != Some(source) {
            #[expect(clippy::cast_possible_wrap, reason = "WebGL enums fit in i32")]
            gl.tex_image_2d_with_u32_and_u32_and_image_bitmap(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                source,
            )?;
            gl.generate_mipmap(Gl::TEXTURE_2D);
            self.uploaded = Some(source.clone());
        }
        self.canvas.resize(output.0, output.1);
        #[expect(clippy::cast_possible_wrap, reason = "canvas sizes fit in i32")]
        gl.viewport(0, 0, output.0 as i32, output.1 as i32);
        let matrix = output_to_source_uv(
            Size {
                w: f64::from(source.width()),
                h: f64::from(source.height()),
            },
            edits,
            crop,
        );
        let values = [
            matrix.a, matrix.b, 0.0, matrix.c, matrix.d, 0.0, matrix.e, matrix.f, 1.0,
        ]
        .map(single);
        gl.uniform_matrix3fv_with_f32_array(Some(&self.locations.output_to_source), false, &values);
        gl.uniform2f(
            Some(&self.locations.output_size),
            single(f64::from(output.0)),
            single(f64::from(output.1)),
        );
        gl.uniform4fv_with_f32_array(Some(&self.locations.background), &[0.0; 4]);
        gl.uniform1fv_with_f32_array(
            Some(&self.locations.adjust),
            &edits.adjust.values().map(|value| single(value / 100.0)),
        );
        gl.draw_arrays(Gl::TRIANGLE_STRIP, 0, 4);
        Ok(())
    }

    pub fn forget(&mut self, source: &ImageBitmap) {
        if self.uploaded.as_ref() == Some(source) {
            self.uploaded = None;
        }
    }
}
