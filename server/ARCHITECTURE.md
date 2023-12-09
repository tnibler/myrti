The main piece of state the application manages is the media catalog:
what assets exist, thumbnails, which representations we have etc.
When something happens like a new file being indexed or another job finishing,
a set of rules determines the next operation that needs to be performed
and applied to the current state.

The database is the single source of truth for the catalog state,
and applying operations (state transitions) to that is easily testable.
Unfortunately state transitions also have side effects,
namely file system changes, ffmpeg invocations and other processing tasks.
To keep things testable, operations are split into two parts:
the side effects that actually do the thing,
and changes to the database (the state transition).

So the flow goes: rules determine operation -> side effects of the operation
are performed -> operation is applied to db (only on success of course).
