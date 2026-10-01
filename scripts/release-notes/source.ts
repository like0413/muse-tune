import { execFileSync, spawnSync } from 'node:child_process'

import { z } from 'zod'

const tagPattern =
  /^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/
const releaseSchema = z.object({
  tag_name: z.string(),
  draft: z.boolean(),
  prerelease: z.boolean(),
})

/** 用参数数组调用 Git，避免把提交内容或标签当作 shell 命令执行。 */
function git(...args: string[]) {
  return execFileSync('git', args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }).trim()
}

/** 校验版本标签；使用完整 ref 消除与分支同名造成的歧义。 */
function resolveTag(tag: string) {
  if (!tagPattern.test(tag)) throw new Error(`Invalid release tag: ${tag}`)
  return git('rev-parse', '--verify', `refs/tags/${tag}^{commit}`)
}

/** 从已发布且属于当前版本历史的 Release 中寻找最近基线。 */
export function collectSource(collectOnly: boolean) {
  const repository = process.env.GITHUB_REPOSITORY
  const tag = process.env.RELEASE_TAG ?? ''
  if (!repository || !/^[\w.-]+\/[\w.-]+$/.test(repository)) {
    throw new Error('Set GITHUB_REPOSITORY to owner/repo.')
  }
  const target = resolveTag(tag)
  if (process.env.SOURCE_COMMIT && process.env.SOURCE_COMMIT !== target) {
    throw new Error('Release tag moved after validation; stop and restart the workflow.')
  }
  const raw = execFileSync(
    'gh',
    ['api', '--paginate', '--slurp', `repos/${repository}/releases?per_page=100`],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  )
  const releases = z.array(z.array(releaseSchema)).parse(JSON.parse(raw)).flat()
  if (!collectOnly && releases.some((release) => release.tag_name === tag && !release.draft)) {
    throw new Error(
      `${tag} is already published. This workflow will not overwrite a public release.`,
    )
  }
  const candidates = releases
    .filter((release) => !release.draft && release.tag_name !== tag)
    .filter((release) => tag.includes('-') || !release.prerelease)
    .filter((release) => tagPattern.test(release.tag_name))
    .map((release) => {
      const commit = resolveTag(release.tag_name)
      const ancestor = spawnSync('git', ['merge-base', '--is-ancestor', commit, target])
      if (ancestor.error || (ancestor.status !== 0 && ancestor.status !== 1)) {
        throw new Error('Unable to inspect release ancestry.')
      }
      return {
        tag: release.tag_name,
        commit,
        distance:
          ancestor.status === 0
            ? Number(git('rev-list', '--count', `${commit}..${target}`))
            : Infinity,
      }
    })
    .filter((release) => Number.isFinite(release.distance) && release.distance > 0)
    .sort((a, b) => a.distance - b.distance || a.tag.localeCompare(b.tag))
  const baseTag = process.env.RELEASE_BASE_TAG || candidates[0]?.tag
  if (!baseTag)
    throw new Error('No previous published ancestor release; set RELEASE_BASE_TAG explicitly.')
  const base = resolveTag(baseTag)
  if (
    spawnSync('git', ['merge-base', '--is-ancestor', base, target]).status !== 0 ||
    base === target
  ) {
    throw new Error('Release base must be a strict ancestor of the target.')
  }
  const range = `${base}..${target}`
  const commits = git('log', '--no-merges', '--format=%H%x00%B%x00', range)
    .split('\0')
    .reduce<{ sha: string; message: string }[]>((result, value, index, values) => {
      if (index % 2 === 0 && value.trim())
        result.push({ sha: value.trim(), message: values[index + 1]?.trim() ?? '' })
      return result
    }, [])
  // 锁文件和二进制不利于归纳用户变化；文件清单仍保留完整范围。
  const diff = git(
    'diff',
    '--no-ext-diff',
    '--no-textconv',
    '--unified=3',
    range,
    '--',
    '.',
    ':(exclude)**/pnpm-lock.yaml',
    ':(exclude)**/Cargo.lock',
    ':(exclude)**/package-lock.json',
    ':(exclude)**/*.svg',
    ':(exclude)**/*.snap',
  )
  const source = {
    repository,
    tag,
    target,
    baseTag,
    base,
    commits,
    files: git('diff', '--name-status', range),
    diff,
  }
  // 保留完整证据；字符数不等于模型 token 数，实际上下文限制由 API 校验。
  return source
}

export type ReleaseSource = ReturnType<typeof collectSource>
