import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import OpenAI from 'openai'

import { renderNotes, validateNotes } from './format.ts'
import { createEditorialPrompt } from './prompt.ts'
import { collectSource } from './source.ts'

/** 先收集可追溯证据，再调用 DeepSeek；失败时不输出可发布的 Markdown。 */
async function main() {
  const collectOnly = process.argv.includes('--collect-only')
  const source = collectSource(collectOnly)
  const directory = resolve(process.env.RELEASE_NOTES_DIR || 'artifacts/release-notes')
  await mkdir(directory, { recursive: true })
  await writeFile(resolve(directory, 'source.json'), JSON.stringify(source, null, 2))
  if (collectOnly) {
    console.log(`Collected ${source.baseTag} → ${source.tag}: ${source.commits.length} commits.`)
    return
  }
  if (!process.env.DEEPSEEK_API_KEY) throw new Error('GitHub secret DEEPSEEK_API_KEY is missing.')
  const model = process.env.DEEPSEEK_MODEL || 'deepseek-flash'
  const client = new OpenAI({
    apiKey: process.env.DEEPSEEK_API_KEY,
    baseURL: 'https://api.deepseek.com',
    timeout: 180_000,
    maxRetries: 2,
  })
  const completion = await client.chat.completions.create({
    model,
    response_format: { type: 'json_object' },
    max_tokens: 16384,
    messages: [
      {
        role: 'system',
        content: createEditorialPrompt(),
      },
      { role: 'user', content: JSON.stringify(source) },
    ],
  })
  const choice = completion.choices[0]
  if (choice?.finish_reason !== 'stop' || !choice.message.content?.trim()) {
    throw new Error('DeepSeek returned empty or incomplete notes; publication is stopped.')
  }
  const notes = validateNotes(JSON.parse(choice.message.content), source)
  await writeFile(resolve(directory, 'notes.json'), JSON.stringify({ model, ...notes }, null, 2))
  await writeFile(resolve(directory, 'release.md'), renderNotes(notes, source))
  console.log(`Validated ${notes.changes.length} Chinese release entries.`)
}

// 不打印 SDK 原始异常，避免服务端响应、请求配置或凭据进入 CI 日志。
main().catch((error: unknown) => {
  console.error(
    error instanceof OpenAI.APIError
      ? `DeepSeek request failed (HTTP ${error.status ?? 'network error'}). Check the secret, quota and model.`
      : error instanceof Error
        ? error.message
        : 'Release notes generation failed.',
  )
  process.exitCode = 1
})
