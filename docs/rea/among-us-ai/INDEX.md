# Among AI — Reference Index

These are static notes on the source tree in `references/among-us-ai` (upstream `duckxy166/among-us-ai`, commit `76c23d7`). Among AI has LLMs play Among Us, in its own simulator or in the real game through the AUBridge mod. The notes study how it wakes an agent at decision points and turns game state into prompts, as a pattern for the Advisor. File paths in the notes are relative to `references/among-us-ai`.

- [Agent state machine](agent-state-machine.html) — how the agent decides from game state: the brain/body split, decision points and their wake reasons, the match and per-seat state machines, the prompt and JSON schema of each state (roam, meeting speak, vote, reflect), output fallbacks, simulator vs real game, and what the Advisor could take from it.
