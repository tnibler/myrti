## Groups (micro-albums in timeline)

I (and most people I've asked) rarely find themselves using the album feature in
their gallery app of choice. I think there are a few reasons for that (purely my opinions):

 - People rarely ever look at their albums, mostly browsing the timeline instead.
 - Creating an album is work without benefit if you never see it, as they are tucked away from the main screen.
 - The term "album", originally designating a physical book, feels appropriate for large collections of images,
 less so for five photos taken on a hike.

A low-threshold way of grouping a small(-ish) number of assets together under a common title,
so that the group is actually shown as such in the timeline
might be a more appropriate way to organize pictures for most scenarios.

For random snapshots, a big timeline grouped by date is nice,
but having shots from a trip actually displayed as "Trip to Rome" in the timeline
would probably make for a nicer browsing experience.
And since you're not "creating an album" but merely changing the grouping in the timeline,
creating such a collection with a title might feel like a lower threshold
process that more people might actually want to go through.

This feature can be supported by a little smartness to make groups even lower friction
by offering a one-click way to group assets detected to be similar in terms of date and location.
For example: 

 - Most assets in the timeline are from location A
 - For a period of time, all assets are at location B,
 - After, assets are back at location A

...could be a pretty good hint that pictures from location B belong together.

Ideas: 

 - Groups are displayed in traditional albums view as well?
 - Display temporally contained albums next to timeline (to the side maybe)

Difference between album and timeline group:
 - timeline groups are always displayed in timeline, albums are not
 - timeline groups should probably mostly contiguous in time
 - assets in album can be sorted arbitratily, timeline groups are chronological

Commonalities between album and timeline group:
 - both contain assets
 - both have a title/description
 - both are displayed in album view

## Timeline

The timeline is grouped by year, month and days as well as groups.
Groupings that have too few assets inside are collapsed into the next higher grouping
(basically the same as all gallery apps).

Groups are roughly at the same level as days and represented the same in the UI,
or maybe they can be below days if a group is contained within one?
Next to the day/group title, the list of locations if applicable is displayed,
every location is clickable (going to the assets by location view).

Questions:
 
 - If some assets in a day are contained in a group and some are not, 
 how is this displayed? E.g., photos in the morning not in group,
 then a group, then more photos not in the group. Options: 

   - Split the day, with the group sitting in between the two parts at the 
   same hierarchy level
   - Move the group to the beginning/end of the day
   - Make the group a sub-item of the day (only works with contiguous groups)
 
 - Non-contiguous groups (somewhat unlikely given their intended use case?):
 the point of a group is to display images together, so it has to be sorted
 under a single date. Probably assign a date to the group based on start/end/... 
 and make the group an item in the hierarchy at the same level of days


## Reverse Geocoding

Assign location to asset

assign common location(s) to group of assets: day in timeline, group, album

browse assets by country/city/location

find assets nearby

future: search any name in geoname database, find assets (if any) close by in Typesense or something

## Media timestamps/timezones

Questions:
 
 - how to handle media with no timezone in metadata/deducible from location tags?
 - how to display datetime in client: local to media, client?
 - how to sort timeline entries from different timezones (example: photos from europe and japan on the "same day")

When extracting media metadata, these are the cases we may encounter:

| Timestamp in metadata                 | GPS Tags? | timestamp_utc | timezone_offset                 | timezone_info          |
|---------------------------------------|-----------|---------------|---------------------------------|------------------------|
| Timestamp with specified offset       | -         | computed      | set                             | `TZ_CERTAIN`           |
| Timestamp in UTC (per QuickTime spec) | no        | set           | `null`                          | `UTC_CERTAIN`          |
| Timestamp in UTC (per QuickTime spec) | yes       | set           | inferred from location          | `TZ_INFERRED_LOCATION` |
| Timestamp with no offset, not UTC     | no        | computed      | guessed to be local             | `TZ_GUESSED_LOCAL`     |
| Timestamp with no offset, not UTC     | yes       | computed      | inferred from location          | `TZ_INFERRED_LOCATION` |
| No timestamp                          | -         | guessed       | guessed to be local             | `NO_TIMESTAMP`         |


## Sharing with other instances

Use case: user/instance A wants to automatically share selected assets with instance B.

Non-copying variant: instance B can query assets from instance A and display them, 
but files stay on instance A.

Copying mode: instance B queries (or is notified of) assets on instance A
and downloads them somwhere, then imports them as if indexed locally
(with reference to the instance where the assets came from).

Resources like transcoded versions are also copied over (barring differences in codec preferences etc),
saving compute.

Questions: 

 - Sharing assets from an album, should the album name be shared?
 - In general, leaking data that is not explicitly shared must be avoided
 (example: faces with assigned names that exist on one instance, but not the other)

## Browsing temporally/spatially related assets
