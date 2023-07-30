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

### /asset/:id/video

GET /asset/:id/video/representations

<!-- GET /asset/:id/video/dashManifest -->

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
