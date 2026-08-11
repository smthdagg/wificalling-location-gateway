# Handoff capsules

Each active Issue uses one `.handoffs/issue-<number>.md` file on its working branch. Copy `.agents/HANDOFF_TEMPLATE.md`, replace every placeholder, commit the capsule with the resumable code, and publish it with `scripts/agent-handoff.sh`.

The exact resumable commit is recorded by the resulting `agent-handoff:v1` GitHub Issue comment. Capsules contain no credentials. An API key belongs only to the Agent's local environment.
