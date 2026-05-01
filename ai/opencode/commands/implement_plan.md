---
description: Implement an approved design document with verification
---

# Implement Plan

You are tasked with implementing an approved design document. These plans contain phases with specific changes and success criteria.

## Getting Started

When given a plan path:
- Read the plan completely and check for any existing checkmarks (- [x])
- Read the original ticket and all files mentioned in the plan
- **Read files fully** - never use limit/offset parameters, you need complete context
- Think deeply about how the pieces fit together
- Create a todo list to track your progress
- By default, create a new branch and new worktree for your work, unless the plan specifies otherwise.
- The default branch name should follow conventional branch structure, e.g., `feature/short-description` or `bugfix/xxxx-###/short-description`, where xxxx is the plan id and ### is the plan number. If the plan specifies a branch name, use that instead.
- The default worktree path should be `./worktrees/name`, where name is derived from the plan title first, then the branch name otherwise. If the plan specifies a worktree path, use that instead.
- Start implementing if you understand what needs to be done

If no plan path provided, ask for one.

## Implementation Philosophy

Plans are carefully designed, but reality can be messy. Your job is to:
- Follow the plan's intent while adapting to what you find
- Implement each phase fully before moving to the next
- Verify your work makes sense in the broader codebase context
- Update checkboxes in the plan as you complete sections

When things don't match the plan exactly, think about why and communicate clearly. The plan is your guide, but your judgment matters too.

If you encounter a mismatch:
- STOP and think deeply about why the plan can't be followed
- Present the issue clearly:
  ```
  Issue in Phase [N]:
  Expected: [what the plan says]
  Found: [actual situation]
  Why this matters: [explanation]

  How should I proceed?
  ```

Always write code documentation for tests, classes, functions, modules, unclear code sections, or system critical paths. If it's obvious what the things does, ask yourself is it clear why the thing exists and how it fits into the bigger picture.

If your code change affects something that is already documented in the code, prefer updated the code documentation already present based on the plan. Then update the plan to reflect the new documentation.

Humans understand writing best when it's presented as a story, narrative, or history. Keep prose optimized for human understanding. Humans understand writing best when it's presented as a story, narrative, or history. Prose should flow like a narrative, not a taxonomy. It should tell the story of why the changes are needed, how they fit together, and what the expected outcome is. Keep code documentation concise and focused on the "why" behind the code, not just the "what". The code blocks should include comments that explain the intent of the code in relation to the overall plan.

Software engineering documentation (SEDs) is defined as the following documentation types:

1. Docstrings and comments in code files for modules, classes, functions, unclear code sections, or system critical paths.
2. Implementation plans
3. Git documentation
4. Research Docs
5. Design Docs
6. Code Documentation

Here are the general questions you should ask yourself for guidance when writing SEDs:

- Why is this important?
- What is this for?
- What is the historical context behind this?
- How does this work?
- How does this fit into the system as a whole and how does it relate to other systems?
- Does the text flow like a narrative that expresses the intent of the requirements or design?

## Verification Approach

After implementing a phase:
- Run the success criteria checks (usually `make check test` covers everything)
- Fix any issues before proceeding
- Update your progress in both the plan and your todos
- Check off completed items in the plan file itself using Edit
- **Pause for human verification**: After completing all automated verification for a phase, pause and inform the human that the phase is ready for manual testing. Use this format:
  ```
  Phase [N] Complete - Ready for Manual Verification

  Automated verification passed:
  - [List automated checks that passed]

  Please perform the manual verification steps listed in the plan:
  - [List manual verification items from the plan]

  Let me know when manual testing is complete so I can proceed to Phase [N+1].
  ```

If instructed to execute multiple phases consecutively, skip the pause until the last phase. Otherwise, assume you are just doing one phase.

do not check off items in the manual testing steps until confirmed by the user.


## If You Get Stuck

When something isn't working as expected:
- First, make sure you've read and understood all the relevant code
- Consider if the codebase has evolved since the plan was written
- Present the mismatch clearly and ask for guidance

Use sub-tasks sparingly - mainly for targeted debugging or exploring unfamiliar territory.

## Resuming Work

If the plan has existing checkmarks:
- Trust that completed work is done
- Pick up from the first unchecked item
- Verify previous work only if something seems off

Remember: You're implementing a solution, not just checking boxes. Keep the end goal in mind and maintain forward momentum.

## Cleaning Up

After completing all phases, present your work to the user, present a commit message, and present a PR message. The commit message should always follow conventional commit structure and by default contain a high level summary of the changes explaining what changed, why it changed, and how it's important written in present tense.

The PR message must contain an # Overview and # Testing section. The overview should mirror the commit message body or have a high level summary of all commit messages in a change.

The testing section should contain a detailed description of how to test the change, including any manual testing steps that need to be performed.

The primary audience for your commit and PR messages is a reviewer or a maintainer. They don't need deep implementation details. They need to understand the high level changes, why they were made, and how to verify them.
