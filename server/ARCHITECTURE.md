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

## Timeline 

Because there is only one timeline (not one per user), we can keep it precomputed
on the server to save time and computation.
Right now, the timeline is implemented as a database view, but it could
be made into a summary table should the need arise.

The structure of the timeline code is built out backwards from how it should be
rendered on the client, largely based on how [Google photos does it](https://web.archive.org/web/20230220194216/https://medium.com/google-design/google-photos-45b714dfbed1).
The timeline is split into sections, which are an (somewhat arbitrary) subdivision
of assets into blocks based on which the total height and scrolling/scrubbing
behavior of the timeline is estimated.
When a section comes into view (or is about to), data for the assets within 
that section is fetched by the client and displayed.
Within a section, there is one or more segments (reusing terminology from the blog post above),
which is a grouping of assets displayed in a contiguous fashion in the timeline
(e.g., a day, location or user-created group). Segments with too few assets 
can be merged by the client for displaying (at least within sections).

An easy approach to create sections would be to just make buckets by month, week
or some other timespan. However, there may be a large number of assets
in a given month for example, requiring large amounts of asset metadata to be fetched
at once. To prevent this, we divide assets into sections such that they are all
suitably long.
A section is then addressed by an index, which the client can use to request
the segments of assets for the sections it needs.

Sections as implemented this way have no meaning to the user at all and should
not affect display in any way, as they are only used for lazy loading and a rough
estimate of scroll height.
As such, segments (which have meaning related to time, location etc.) can overlap
section boundaries, since the client can just merge them if it notices that the
last segment of a loaded section and the first segment of a newly fetched section 
belong together.
For segments based on date or timeline groups, it's easy to keep them contained
in a single section even in SQL, so we can avoid all of this right now.
In the future, segments may be created automatically based on very smart logic
which is less easily integrated with the basic timeline section SQL view.

### Handling changes in timeline while viewing

Realistic option: reload everything. Sensible

Middle ground, robust to simple changes: 
if between fetching of all sections (ids, number of assets, date range)
and the segments/assets within the section the date range hasn't changed,
we don't have to do anything.
If the start and end date of the section have changed, probably simplest 
to just reload and rebuild. If only one of start/end dates has changed,
maybe only a partial reload of some sort is necessary. 
