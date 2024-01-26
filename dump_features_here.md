## Meaningful

 - something smaller than an album, just a group of pictures with
 common description in the timeline (not sure how that would look,
 all I know I very rarely actually use albums, but would like to label
 a couple of pictures from the same trip/event as such and have them
 kind of shown as a group in the timeline)
 - Map with pins of photos on it. Tap on a pin, open up to scroll
 through photos from the same time and place, then further away
 temporally and spatially as you scroll but still relevant
 - A way of creating a representative collection of photos from
 a certain timespan

## Details I don't want to forget

 - multiple remotes on the same client, switch between them (ie family and personal photos)
 - no authentication required from local/wireguard interfaces, enable login when accessing from outside
 - transcoding/processing on different client with better hardware when it makes itself available
 - Automatic database backup, recovery if data dirs get corrupted/lost
 - deduplication: hash during indexing, mark duplicate of
 - click on album -> download as zip/tar, with a self-contained gallery html file
 - store perceptive hash with assets and dedupe if possible


client GET /dash/:id/stream.mpd         -> d/dash/:id/stream.mpd
client GET /dash/:id/av1_1080x1920.mp4  -> d/dash/:id/av1_1080x1920.mp4
assemble key dash/:id/:file

shaka-packager local_file,output=av1_1080x1920.mp4 av1_1080x1920.mp4.media_info
needs to be run in output directory so that media_info contains filename instead of path
if localfilestorage: create path, cd into directory, run in cwd
otherwise: cd into tempdir, run in cwd, write to storage_provider

client hints in what context it's currently looking at an asset, so the server 
can prefetch the next ones (in the timeline for instance) from disk
