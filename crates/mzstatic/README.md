# mzstatic URLS

mzstatic is a wretched black box of a domain used by Apple to serve various types of content.

## Anatomy

The format of a URL can vary, but one frequently seen format is as follows:

```

                                              ┌─ Asset Token
                                           ┌──┴──────────────────────────────────────────────────────────────────┐
- https://is1-ssl.mzstatic.com/image/thumb/Music116/v4/c7/65/ff/c765ff9c-3757-4b96-043b-b9551d96d731/pr_source.png/632x632SC.FPINF03-70.webp?l=en-US
 ┌┴───┘   └──┬──┘ └──┬───┘     └─┬───────┘ └┬─────┘ └───┬──────────────────────────────────────────┘ └┬──────────┘ └────────┬──────────────────────┘
 │           │       │           │          │           │                                             │                     │
 └─ Protocol │       │           │          └─ Pool     └─ Main Token Path (UUID Variant)             └─ Original Filename  └─ Thumbnail Parameters
             │       │           │          
             │       └─ Domain   └─ Prefix 
             │
             └─ Subdomain
```

### Protocol & Subdomain

Subdomains are typically one to two letters, usually followed by some sort of load-balancing number.

In the case of the "is" variant, only HTTP is supported on the primitive subdomain variants, but when suffixed by "-ssl", HTTPS is enforced.

#### Known *mzstatic* Subdomains

- `/^a[1-5]$/` - High-quality non-thumbnail image distribution.
- `/^is[1-5](?:-ssl)$/` - Generated thumbnail or otherwise dynamically edited images.
- `/^[rs]$/` - Purpose unknown; seen hosting HTML assets like icons in a PDF format for "da-storefront".
- `/^s[1-5]$/` - Purpose unknown; seen hosting JSON data for the drone footage that scrolls in the background of an Apple TV. Not interchangeable with single-letter 's'.
- `/^apps$/` - Purpose unknown; seen hosting an Android manifest and a (broken?) web build for some sort of Apple web-app.
- `/^itc$/` - Purpose unknown.

Only the first two URLs will be covered in the document in crate.

### Domain

Several other domains also internally redirect to the same backend as "mzstatic".

- http://a1.phobos.apple.com/us/r1000/000/Features/atv/AutumnResources/videos/comp_HK_H004_C008_v10_6Mbps.mov
- https://a1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/comp_HK_H004_C008_v10_6Mbps.mov

It can act a bit odd, as demonstrated by attempting to access the second link.

Only the "mzstatic" URL will be covered in this document and crate.

### Prefix

TODO: re-write for accelerator directives, add those to example anatomy

#### `/image/thumb`

- This section is present only in `/^is[1-5](?:-ssl)$/` subdomains. It seems to specify that a thumbnail is being retrieved.
- If you remove this part of the URL and swap the subdomain to one satisfying `/^a[1-5]$/` whilst removing the thumbnail payload, it will return a lossless(?) version of the image.
- It is always accompanied by a relevant ["Thumbnail Parameters"](#thumbnail-parameters) payload, except in the case of `/image/thumb/gen/`, which will be discussed later.

#### Unknown Variant

To be determined. Involves a region code, which seemingly doesn't affect anything.

- https://a3.mzstatic.com/us/r30/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg
    - Reformatted:
    - https://is1-ssl.mzstatic.com/image/thumb/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png
    - https://is1-ssl.mzstatic.com/image/pf/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png
- http://is5.mzstatic.com/image/pf/us/r30/Music4/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.jpg
- https://a5.mzstatic.com/us/r1000/0/Music122/v4/c8/03/57/c803571e-6d17-f10f-fddf-fd4f7fc00d5e/22UMGIM37441.rgb.jpg
- https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json
    - Not relevant to use-case, but the `000` is interesting in that it cannot be replaced with `0`.
        - The error message seems to indicate that it isn't considered as part of the token.
    - Trying `r30` causes a null token error.

### Pool

The pool is what category the image generally falls under (i.e. for what service it is relevant to), optionally followed by some type of non-zero positive integer sub-marker, so far always under 256.

It is typically present in `/^is[1-5](?:-ssl)$/` and `/^a[1-5]$/` subdomains, but not always.










Some examples of where this does not apply can be seen in the ["Main Token Path"](#main-token-path) section.

#### Variants

- `Purple` - App Store
- `PurpleSource` - App Store
- `Podcasts` - Apple Podcasts.
- `AMCArtistImages` - Apple Music artist images; the meaning behind the 'C' is unknown.
- `Features` - Various "featured" content.
- `Video` - Apple TV?
- `Music` - Apple Music.
- `Publication` - Apple Books.
- `FuseSocial` - Unknown; seen on videos on Apple Music.
- `CobaltPublic` - Unknown; seen on educational PDFs. Associated with iTunes U.

### Main Token Path

In the case of `/^is[1-5](?:-ssl)$/` and `/^a[1-5]` subdomains, this typically follows the following format:

1. "v4"
2. 2 characters: the 1st two characters of a UUID
3. 2 characters: the 2nd two characters of a UUID
4. 2 characters: the 3rd two characters of a UUID
5. A UUID (no true version or variant, despite the "v4"), with the first six characters matching what came earlier.

For all other subdomains, order can be regarded as going out the window.

Even within the relevant subset, the path does not remain consistent.

Here are several examples of where this does not apply:

<!-- Potentially relevant: the ability to turn them (or rather, *not*) into /^a[1-5]$/ subdomains? --> 
- https://is2-ssl.mzstatic.com/image/thumb/Music/0a/1f/85/mzi.adskaamt.tif/600x600bb.jpg <!-- "mzi.adskaamt" is cloud ID? -->
- https://is1-ssl.mzstatic.com/image/thumb/Music/y2005/m07/d15/h00/s05.zcmaenrh.tif/600x600bb.jpg
- https://is1-ssl.mzstatic.com/image/thumb/gen/600x600AM.PDCXS01.jpg (Apple Music generated playlist cover; covered later.)
- https://is1-ssl.mzstatic.com/image/thumb/BwzocRAAEgP6sUF5tTNW9w/492x492ve.webp (Apple TV "Cast and Crew")
<!-- ^^ this is also present in my artwork db for the avatar of the random account on my shit -->
- https://is1-ssl.mzstatic.com/image/thumb/dGLqT-f-3AHL34XWpgSf-Q/256x256bb.jpg (Apple Music user icon)


### Thumbnail Parameters

Not present on `/^a[1-5]$/` servers.

For now, see code for further documentation.

<!-- shit like this `SG-MQ-US-032-Image000001/v4/28/34/25/28342536-9ff3-5a2f-5afa-162612ffb940/image` is a valid token in the DB but no associated URL ?????????? Why man. Why.  wait. us 032, ,,,,, us,,,, that reminds me of the accelerator directive shit >

### Thumbnail Details

```

TODO add quality
632x632SC.FPINF03.webp?l=en-US
└┬────┘└┬───────┘ └┬─┘ └──┬──┘
 │      │          │      └─ Language
 │      └─ Effect  │
 │                 └─ Format
 └─ Resolution
```
TODO

### Generated Thumbnail

TODO

## Other Information

### HTTP Header Details
- Intact "last-modified", sometimes "date" too? At least on `/^a[1-5]` subdomains.
