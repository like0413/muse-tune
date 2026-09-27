# 自动生成中文发布说明

## 一次性配置

在应用仓库 Settings → Secrets and variables → Actions → Secrets 添加
`DEEPSEEK_API_KEY`，值为 DeepSeek API 密钥。不要提交密钥或写到前端环境变量。

默认模型是 DeepSeek 官方文档当前的 `deepseek-flash`；如需更换，在同一位置的
Variables 配置 `DEEPSEEK_MODEL`。服务地址固定为 `https://api.deepseek.com`。
`openai` 只是 DeepSeek 官方示例使用的兼容客户端 SDK，不使用 OpenAI 密钥或服务。

## 发布流程

继续使用原来的 `vp run release` 提交版本并推送标签：

1. 校验标签、签名、版本并构建共享前端。
2. 按当前标签的精确提交收集上个已发布版本以来的提交信息、文件清单与代码净差异。
3. DeepSeek 将用户可见变化整理成中文“新增 / 变更 / 修复”，Zod 校验结构与来源提交。
4. 构建安装版、便携版；安装版任务创建暂未公开的 Release，并上传更新器清单。
5. 确认两种包完整、更新清单有效且说明一致后，自动公开 Release，**不需要人工审核**。

生成失败、空响应、截断、无效 JSON、引用范围之外的提交或任一构建失败，都会阻止公开。
API 对连接问题、限流及服务端错误最多自动重试两次；内容校验失败不会循环重新生成。
格式与来源检查不能保证 AI 对代码的语义判断完全准确。

提交前缀不直接对应展示标签：`perf` 的用户可见改善归入“变更”，纯 `refactor`、
`chore`、`ci` 等通常省略，但若改变安装、更新、兼容性或默认行为仍需展示。
页面只保留“新增 / 变更 / 修复”，不为每种 Conventional Commit 类型增加标签。

非发布构建（手动运行时 `publish=false`）不会调用 DeepSeek，也不要求它的密钥。
现有 Tauri 签名要求保持不变。

## 版本范围与重跑

- 正式版：选择当前提交历史里最近的已发布正式版，避免正式版遗漏预发布阶段的变化。
- 预发布：允许选择最近的已发布预发布版作为基线。
- 排除草稿、当前标签、非祖先提交；不使用最新 main 的未发布代码。
- 工作流工具取自工作流提交，产品证据和安装包取自校验过的标签提交。
- 未发布的失败草稿可以重跑；会重新生成说明、同步正文并覆盖构建资产。
- **已公开的版本拒绝覆盖**。这套流程从下一个版本生效，不会补写 v1.0.1 的说明。
- 若没有历史基线，脚本会停止；可通过 `RELEASE_BASE_TAG` 显式指定已有祖先标签。
- 证据超过 180000 字符会停止，不静默截断。需要拆分发布或调整证据整理方案。

## 本地只检查证据，不调用 AI

先确保 `gh auth login` 已完成，并执行 `git fetch --tags` 获取历史。
PowerShell 示例：

```powershell
$env:GITHUB_REPOSITORY = 'like0413/muse-tune'
$env:RELEASE_TAG = 'v1.0.1'
vp run release:notes --collect-only
```

这个模式也允许读取已发布版本，但不会修改远端 Release。输出位于已被 Git 忽略的
`artifacts/release-notes/source.json`。移除 `--collect-only` 才会调用 DeepSeek；正常发布交给 CI。

## 产物与网站约定

Actions 的 `release-notes-<commit>` artifact 保留 30 天：

- `source.json`：版本范围、提交、文件清单、净差异。
- `notes.json`：模型名称、中文条目及每条对应的来源提交。
- `release.md`：真正发布的 Markdown，同时用于 Tauri `latest.json` 的 notes。

GitHub Release 正文是给网站消费的公开内容源，格式固定为：

```markdown
## 新增

- **功能标题**：说明用户能做什么。

## 变更

- **变更标题**：说明行为变化和必要的升级注意事项。

## 修复

- **修复标题**：说明修复了什么用户可见问题。
```

空分类不输出，末尾附完整版本对比链接。中文站、英文站都使用同一份中文正文。
网站数据接入与部署通知属于另一仓库；本流程暂未配置跨仓库通知。
未来应在 `publish` 成功后显式触发网站部署，不能依赖本流程的 `GITHUB_TOKEN`
触发另一个 `release: published` 工作流。

## 官方依据

- [DeepSeek 官方 SDK 示例](https://api-docs.deepseek.com/)
- [DeepSeek JSON Output](https://api-docs.deepseek.com/guides/json_mode/)
- [Tauri Action 的 releaseBody 与发布选项](https://github.com/tauri-apps/tauri-action)
- [GitHub 工作流触发限制](https://docs.github.com/en/actions/how-tos/writing-workflows/choosing-when-your-workflow-runs/triggering-a-workflow)
