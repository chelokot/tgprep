import { ADJUSTMENT_KEYS, type Crop, type Edits } from '../model/types'
import { outputToSourceUv, type Size } from './geometry'

const VERTEX_SHADER = `#version 300 es
in vec2 position;
out vec2 outputUv;
void main() {
  outputUv = vec2(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
  gl_Position = vec4(position, 0.0, 1.0);
}`

const FRAGMENT_SHADER = `#version 300 es
precision highp float;
in vec2 outputUv;
out vec4 color;
uniform sampler2D image;
uniform mat3 outputToSource;
uniform vec2 outputSize;
uniform vec4 background;
uniform float adjust[${ADJUSTMENT_KEYS.length}];
${ADJUSTMENT_KEYS.map((key, index) => `#define ${key.toUpperCase()} adjust[${index}]`).join('\n')}

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
}`

type Source = ImageBitmap

type Locations = Record<'outputToSource' | 'outputSize' | 'background' | 'adjust', WebGLUniformLocation>

const compile = (gl: WebGL2RenderingContext, type: number, code: string) => {
  const shader = gl.createShader(type)!
  gl.shaderSource(shader, code)
  gl.compileShader(shader)
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? 'shader')
  return shader
}

export class Renderer {
  private readonly gl: WebGL2RenderingContext
  private readonly locations: Locations
  private uploaded: Source | null = null
  readonly maxTextureSize: number
  readonly canvas: HTMLCanvasElement | OffscreenCanvas

  constructor(canvas: HTMLCanvasElement | OffscreenCanvas) {
    this.canvas = canvas
    const gl = canvas.getContext('webgl2', { premultipliedAlpha: false, preserveDrawingBuffer: true, antialias: false })
    if (!gl) throw new Error('WebGL2 is not available')
    this.gl = gl as WebGL2RenderingContext
    const program = this.gl.createProgram()
    this.gl.attachShader(program, compile(this.gl, this.gl.VERTEX_SHADER, VERTEX_SHADER))
    this.gl.attachShader(program, compile(this.gl, this.gl.FRAGMENT_SHADER, FRAGMENT_SHADER))
    this.gl.linkProgram(program)
    this.gl.useProgram(program)

    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.gl.createBuffer())
    this.gl.bufferData(this.gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), this.gl.STATIC_DRAW)
    const position = this.gl.getAttribLocation(program, 'position')
    this.gl.enableVertexAttribArray(position)
    this.gl.vertexAttribPointer(position, 2, this.gl.FLOAT, false, 0, 0)

    this.gl.bindTexture(this.gl.TEXTURE_2D, this.gl.createTexture())
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MIN_FILTER, this.gl.LINEAR_MIPMAP_LINEAR)
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MAG_FILTER, this.gl.LINEAR)
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_S, this.gl.CLAMP_TO_EDGE)
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_T, this.gl.CLAMP_TO_EDGE)

    const locate = (name: string) => this.gl.getUniformLocation(program, name)!
    this.locations = {
      outputToSource: locate('outputToSource'),
      outputSize: locate('outputSize'),
      background: locate('background'),
      adjust: locate('adjust'),
    }
    this.maxTextureSize = this.gl.getParameter(this.gl.MAX_TEXTURE_SIZE)
  }

  render(source: Source, edits: Edits, crop: Crop, output: Size, background: [number, number, number, number] = [0, 0, 0, 0]) {
    const { gl, locations } = this
    if (this.uploaded !== source) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source)
      gl.generateMipmap(gl.TEXTURE_2D)
      this.uploaded = source
    }
    this.canvas.width = output.w
    this.canvas.height = output.h
    gl.viewport(0, 0, output.w, output.h)
    const matrix = outputToSourceUv({ w: source.width, h: source.height }, edits, crop)
    gl.uniformMatrix3fv(locations.outputToSource, false, [matrix.a, matrix.b, 0, matrix.c, matrix.d, 0, matrix.e, matrix.f, 1])
    gl.uniform2f(locations.outputSize, output.w, output.h)
    gl.uniform4fv(locations.background, background)
    gl.uniform1fv(locations.adjust, ADJUSTMENT_KEYS.map((key) => edits.adjust[key] / 100))
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
  }

  forget(source: Source) {
    if (this.uploaded === source) this.uploaded = null
  }
}
