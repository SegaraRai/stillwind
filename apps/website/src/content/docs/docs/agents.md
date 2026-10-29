---
title: Coding agents
description: Let a coding agent set up Stillwind or migrate a project, using the documentation and skills in the stillwind package.
slug: docs/agents
---

The `stillwind` package includes this documentation as Markdown files in
`node_modules/stillwind/docs/`. It also includes skills for coding agents. An
agent that reads these files uses the documentation of the installed version.

## Start with a prompt

Give this prompt to your agent in a new or an existing project:

```text
Set up Stillwind in this project. If the stillwind package is not installed,
install it with the project's package manager. Then read
node_modules/stillwind/docs/agents.md and follow its instructions for agents.
```

The agent decides whether the project needs a new setup or a migration. Before
it migrates existing styles, it explains the options and asks how far to go.

## What the package includes

| Path                           | Content                                                      |
| ------------------------------ | ------------------------------------------------------------ |
| `docs/`                        | This documentation as Markdown                               |
| `skills/setup-stillwind/`      | A skill that sets up Stillwind in a project without styles   |
| `skills/migrate-to-stillwind/` | A skill that migrates Tailwind CSS or other CSS to Stillwind |
| `skills/stillwind/`            | A skill for everyday work in a project that uses Stillwind   |
| `AGENTS_TEMPLATE.md`           | A section for the instruction file of your project           |

## Instructions for agents

1. Inspect the project. Find the package manager, the build tool, the framework,
   and the current styling. In a monorepo, find the application that the user
   means.
2. Choose a skill and follow its `SKILL.md`. The skills are in
   `node_modules/stillwind/skills/`:
   - Use `setup-stillwind` when the project has no styles to migrate, such as a
     new project.
   - Use `migrate-to-stillwind` when the project uses Tailwind CSS, plain CSS,
     CSS Modules, a preprocessor, or CSS-in-JS.
3. Tell the user which skill you chose and why, in one sentence.

For later work in a project that uses Stillwind, follow `skills/stillwind/SKILL.md`.

## Install the skills

Agents can read the skills from `node_modules/stillwind/skills/` without
installation. To let your agent load them automatically, link them into its
skill directory, such as `.claude/skills/` for Claude Code or `.agents/skills/`
for Codex:

```sh
mkdir -p .claude/skills
ln -s ../../node_modules/stillwind/skills/stillwind .claude/skills/stillwind
```

A link keeps the skill in sync with the installed version. Copy the directory
instead when your system does not support links.

## Add project instructions

Add the contents of `node_modules/stillwind/AGENTS_TEMPLATE.md` to the instruction
file of your project, such as `AGENTS.md` or `CLAUDE.md`. The section tells
agents in later sessions that the project uses Stillwind and where its
documentation is.

## Read the documentation online

Every page of this website has a **View as Markdown** link. Give your agent the
[documentation index](/docs/llms.txt), or retrieve the
[full documentation](/llms-full.txt) in one request.
