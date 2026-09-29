import { z } from 'zod'

import type { ReleaseSource } from './source.ts'

const text = z
  .string()
  .trim()
  .min(1)
  .regex(/^[^\r\n<>]+$/)
export const notesSchema = z.strictObject({
  changes: z
    .array(
      z.strictObject({
        kind: z.enum(['added', 'changed', 'fixed']),
        // 上限留出必要的产品名和升级注意事项；超长时拒绝发布，不截断文字。
        title: text.max(40),
        description: text.max(160),
        sources: z.array(z.string().regex(/^[a-f0-9]{40}$/)).min(1),
      }),
    )
    .max(30),
})

/** 校验结构和来源；不能代替对语义真实性的判断。 */
export function validateNotes(value: unknown, source: ReleaseSource) {
  const notes = notesSchema.parse(value)
  const known = new Set(source.commits.map((commit) => commit.sha))
  for (const change of notes.changes) {
    if (change.sources.some((sha) => !known.has(sha)))
      throw new Error('AI cited a commit outside this release.')
    if (!/\p{Script=Han}/u.test(change.title + change.description)) {
      throw new Error('Chinese release notes are missing.')
    }
  }
  return notes
}

/** 将模型文字按纯文本转义，Markdown 结构由代码统一生成。 */
function escapeMarkdown(value: string) {
  return value.replace(/([\\`*_{}[\]()#+.!|~>@])/g, '\\$1')
}

/** 固定中文分类，网站从同一份 Release body 提取条目，不生成翻译副本。 */
export function renderNotes(notes: z.infer<typeof notesSchema>, source: ReleaseSource) {
  const sections: string[] = []
  const headings = ['新增', '变更', '修复']
  for (const [index, kind] of ['added', 'changed', 'fixed'].entries()) {
    const changes = notes.changes.filter((change) => change.kind === kind)
    if (!changes.length) continue
    sections.push(`## ${headings[index]}`)
    sections.push(
      changes
        .map((change) => {
          return `- **${escapeMarkdown(change.title)}**：${escapeMarkdown(change.description)}`
        })
        .join('\n'),
    )
  }
  if (!notes.changes.length) sections.push('本次版本没有需要单独说明的用户可见变更。')
  sections.push(
    `**Full Changelog**: https://github.com/${source.repository}/compare/${source.baseTag}...${source.tag}`,
  )
  return `${sections.join('\n\n')}\n`
}
