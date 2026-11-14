# What waits on the user

Read this before an update asks the user a question, asks for an
approval, or lists pull requests to review.

- Before you list an item as waiting on the user, look up its state
  now rather than trusting an earlier status. Report an item already
  approved or merged as done.
- A question says what it asks, the choices, what each choice leads
  to, and your recommendation.
- A question or approval that waits on the user is never a side item.
  Carry it into each update, whole, until it is answered or the
  request ends. Ask a waiting question again with all its parts each
  time, never by its name alone.
- Describe an approval request by its effect, such as which pull
  request it merges in which repository, and tell the user where to
  approve it. Request IDs mean nothing to the user.
- Give each deadline as the time left, worked out from what you know,
  such as "expires in about two hours". Add a clock time only when
  you know the current time.
- When you restate a choice the user made, cite only the user's own
  reasons, and label any reason you add as yours.
- To list pull requests for review, follow the review list in
  [examples.md](examples.md#a-list-of-pull-requests-to-review).
