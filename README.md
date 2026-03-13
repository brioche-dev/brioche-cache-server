# brioche-cache-server (archived)

This project was part of the migration to Brioche's self-hosted infrastructure. The goal was to reduce the number of calls to our S3 provider for hosting the Brioche cache at <https://cache.brioche.dev>. Basically, this server acts as a caching proxy in front of S3, but limited in scope to the S3 objects that Brioche uses.

This turned out to be really successful! But we also had a few other miscellaneous publicly-hosted S3 buckets too, which I also wanted to set up a cache in front of.

So, I split off the work from this repo into its own standalone project, and made it work as a more general HTTP / S3 caching reverse proxy. The new version is now called [`server3`](https://github.com/kylewlacy/server3), and fully covers the use case that `brioche-cache-server` did, but it's much more flexible and customizable. `server3` is still fairly specialized, but it works for _a lot_ more use cases than `brioche-cache-server` did!
