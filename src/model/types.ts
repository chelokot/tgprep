import { z } from 'zod'

export const MAX_POST_PHOTOS = 10
export const MAX_CAPTION_LENGTH = 1024

export const ADJUSTMENT_KEYS = [
  'exposure',
  'brightness',
  'contrast',
  'highlights',
  'shadows',
  'temperature',
  'tint',
  'saturation',
  'vibrance',
  'sharpness',
  'vignette',
  'fade',
  'grain',
] as const

export type AdjustmentKey = (typeof ADJUSTMENT_KEYS)[number]

export const ASPECTS = ['free', 'original', '1:1', '4:5', '5:4', '3:4', '4:3', '2:3', '3:2', '9:16', '16:9'] as const

export type Aspect = (typeof ASPECTS)[number]

const cropSchema = z.object({ x: z.number(), y: z.number(), w: z.number(), h: z.number() })

const editsSchema = z.object({
  crop: cropSchema,
  aspect: z.enum(ASPECTS),
  angle: z.number(),
  quarter: z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3)]),
  flip: z.boolean(),
  adjust: z.object(Object.fromEntries(ADJUSTMENT_KEYS.map((key) => [key, z.number()])) as Record<AdjustmentKey, z.ZodNumber>),
})

const photoMetaSchema = z.object({
  checked: z.boolean(),
  archived: z.boolean(),
  category: z.string().nullable().default(null),
  edits: editsSchema,
})

const postSchema = z.object({
  id: z.string(),
  category: z.string(),
  items: z.array(z.string()),
  caption: z.string(),
  sent: z.boolean(),
})

const categorySchema = z.object({ id: z.string(), name: z.string(), color: z.string() })

export const docSchema = z.object({
  app: z.literal('tgprep'),
  version: z.literal(1),
  categories: z.array(categorySchema).min(1),
  posts: z.array(postSchema),
  photos: z.record(z.string(), photoMetaSchema),
})

export type Crop = z.infer<typeof cropSchema>
export type Edits = z.infer<typeof editsSchema>
export type PhotoMeta = z.infer<typeof photoMetaSchema>
export type Post = z.infer<typeof postSchema>
export type Category = z.infer<typeof categorySchema>
export type Doc = z.infer<typeof docSchema>
export type Adjustments = Edits['adjust']

export const FULL_CROP: Crop = { x: 0, y: 0, w: 1, h: 1 }

export const DEFAULT_ADJUSTMENTS = Object.fromEntries(ADJUSTMENT_KEYS.map((key) => [key, 0])) as Adjustments

export const DEFAULT_EDITS: Edits = {
  crop: FULL_CROP,
  aspect: 'free',
  angle: 0,
  quarter: 0,
  flip: false,
  adjust: DEFAULT_ADJUSTMENTS,
}

export const DEFAULT_META: PhotoMeta = { checked: false, archived: false, category: null, edits: DEFAULT_EDITS }

export const CATEGORY_COLORS = ['#3390ec', '#e5487d', '#30a46c', '#f5a524', '#8e4ec6', '#12a594'] as const

export const EMPTY_DOC: Doc = {
  app: 'tgprep',
  version: 1,
  categories: [
    { id: 'friends', name: 'Друзьям', color: CATEGORY_COLORS[0] },
    { id: 'family', name: 'Маме', color: CATEGORY_COLORS[1] },
  ],
  posts: [],
  photos: {},
}

export const metaOf = (doc: Doc, id: string): PhotoMeta => doc.photos[id] ?? DEFAULT_META

export const photoCategory = (doc: Doc, id: string) => metaOf(doc, id).category ?? doc.categories[0].id

export const hasAdjustments = (adjust: Adjustments) => ADJUSTMENT_KEYS.some((key) => adjust[key] !== 0)

export const hasGeometry = (edits: Edits) =>
  edits.angle !== 0 ||
  edits.quarter !== 0 ||
  edits.flip ||
  edits.crop.x !== 0 ||
  edits.crop.y !== 0 ||
  edits.crop.w !== 1 ||
  edits.crop.h !== 1

export const isEdited = (edits: Edits) => hasGeometry(edits) || hasAdjustments(edits.adjust)

export type PhotoFile = {
  id: string
  name: string
  handle: FileSystemFileHandle
  size: number
  lastModified: number
}
