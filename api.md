GET /serverId

probably useful for clients to keep track of multiple remotes

GET /initialSetup

true if no asset roots have been added yet -> let user pick from file tree in browser
 
GET /stats

GET /ping

## /assetRoot

GET /assetRoot

GET /assetRoot/:id

POST /assetRoot/:id/reindex

## /asset

GET /asset

GET /asset/:id

GET /asset/thumbnail/:id
size="small"/"large"
aspect="original"/"square"
format

GET /asset/file/:id
assetId

GET /asset/countByTimeSlice
timeSpan="month"/"week"

GET /asset/byTimeSlice
start: date
end: date

GET /asset/timeline
startId: string 
count: number
lastFetch: datetime (if new assets since then, we have to refetch/update those)

response:
changedSinceLastFetch: bool
assets: {[]}

### /asset/:id/video

GET /dash/:id/stream.mpd

maybe special case? otherwise just serve files under /dash/:id/

GET /dash/:id/:segment

look for file with name :segment in DashResourceDir for asset

## /album

GET /album

GET /album/:id

GET /album/assets
albumId

GET /album/assetRange
albumId
indexStart: int
count: int

POST /album/create
name
assetIds

POST /album/addAssets
albumId
assetIds

## /job

GET /jobs
status="running"/"complete"/"failed..

GET /jobs/:id
