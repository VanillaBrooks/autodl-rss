# autodl-rss
Automatic torrent management from RSS feeds

## How It Works

Using a config.yaml file, `autodl-rss` will parse RSS feeds at specified intervals for user-specific tag / title keywords. A RSS item that matches the given title and tag conditions will be forwarded to the torrent client to be downloaded.

## config.yaml usage

autodl-rss uses a config.yaml file to define: 
* what kinds of content should be downloaded
* what conditions should it meet
* where should the torrent be stored
* when to pause torrents in the client

### Matchers:

```
example_matcher: &reference_tag
  tags_wanted:                                                  # OPTIONAL
    - -  # every line describes an AND condition
    - [] # things in brackets with a single dash represent OR conditions
  tags_banned:                                                  # OPTIONAL
    - -  # ...
  title_wanted:                                                 # OPTIONAL
    - -  # ..
    - [] # ..
  title_banned:                                                 # OPTIONAL
    - -  # ...
    - [] # ...
  save_folder: # location for the torrent to be downloaded to   # MANDATORY
```
### Feeds
Feeds specify how all the matchers fit together
* What matchers apply to what feed
* urls to each RSS feed
* rate at which the feed is checked

```
feeds:
  - url: https://rss_feed_url ... 
    update_interval: <frequency feed is checked in seconds>
    matchers:
      - *matcher1
      - *matcher2
  - url: https://another_rss_feed ...
    update_interval: # '' 
    matchers:
      - *matcher1               # references the "&" tag of each matcher
      - *matcher3               # matchers can be reused by different feeds
```

### trackers_to_keep

A tracker URL matching ANY of the items in the list will never be paused

```
trackers_to_keep
 - rarbg.to
 - nyaa.si
```


## Example config.yaml

```
# config.yaml in same working directory as executable

# first matcher, referencable by "*matcher_1" 
# each matcher tries to find a torrent in the RSS feed with the same tag and title conditions
# each mathcher has a customizable "save_folder" where the torrent will be downloaded to

example_matcher: &matcher_1
  tags_wanted
    - [condition1, condition2]      # will filter to ensure the torrent is tagged is condition1 OR condition2
    - - condition3                  # torrent must also have condition 3
    - [condition4]                  # torrent must also have condition 4 (alternate syntax)
  tags_banned:
    - [tag1, tag2]                  # dont download anything with (tag1 || tag2)
  save_folder: path/to/torrent/download/location


# seconds matcher, referencable by "*matcher_2"

example_matcher2: &matcher_2
  tags_wanted:                              # torrent must have (tag1 || tag 2) && other_tag_keyword
    - [tag2, tag1]              
    - - other_tag_keyword
  title_wanted:
    - - title_condition1
    - [title_keyword2, title_keyword3]      # torrent title must have EITHER keyword2 or keyword3
  title_banned:                 
    - - banned_title1                       # torrent will not be downloaded if either 
    - - banned_title2                       # banned_title1 or banned_title2 are present
  save_folder: path/to/torrent/download/location


feeds:
    - url : https://rss_tracker_url_here.com/...    # RSS feed to download from
      update_interval: 600                          # check feed every 600 seconds
      matcher:                                      # different matching conditions to check this RSS feed for
        - *matcher_1
        - *matcher_2
    - url: https://another_rss_url.com/...          # can scan multiple urls 
      update_interval: 60                           # update every 60 seconds
      matcher:
        - *matcher_1                                # can reuse matchers for differnet RSS feeds

# torrents from these trackers will not be auto-paused when completed
trackers_to_keep:
    - rarbg.to
    - nyaa.si
```

## Notes

* all tags / titles should be lowercase
    * all rss feed keywords are converted to lowercase before being checked against the `matcher`s
* project currently assumes usage of qbittorrent 
    * enabled web interface
    * localhost does not require password
    * hosted at port 8080
        * `http://localhost:8080` should bring the web ui
    * differnet clients to be supported in the future

* works generally with private tracker RSS feeds using Gazelle
    * many public trackers, such as rarbg.to, will also work
    * some RSS feeds (without torrent download links) or different RSS feed layouts may not be compatable
        * more on this later