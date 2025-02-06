#![allow(unused)]
pub mod accelerator;
pub mod pool;
pub mod image;

// todo: refactor quality to be struct to Make Invalid State Unrepresentable

macro_rules! read {
    ($slice: ident, while: $expr: expr) => {
        {
            let length = $slice.chars().take_while($expr).count();
            let out = &$slice[..length];
            *(&mut $slice) = &$slice[length..];
            out
        }
    };
    ($slice: ident, delimit: $needle: expr) => {
        {
            if let Some(at) = $slice.find($needle) {
                Some(read!($slice, delimit_at: at, $needle.len()))
            } else { None }
        }
    };
    ($slice: ident, delimit char: $needle: expr) => {
        {
            if let Some(at) = $slice.find($needle) {
                Some(read!($slice, delimit_at: at, $needle.len_utf8()))
            } else { None }
        }
    };
    ($slice: ident, delimit_at: $delimit_at: expr, $skip_amount: expr) => {
        {
            let value = &$slice[..$delimit_at];
            *(&mut $slice) = &$slice[($delimit_at + $skip_amount)..];
            value
        }
    };
    ($slice: ident, delimit_at: $delimit_at: expr) => {
        read!($slice, delimit_at: $delimit_at, 1)
    }
}

pub(crate) use read;

    

// https://is1-ssl.mzstatic.com/image/thumb/gen/600x600AM.PDCXS01.jpg?c1=FFFFFF&c2=CCA3A3&c3=960019&c4=1A1414&signature=cd00baed652789cfa36f326160fcf46c7786df4366fd6f2fbd189bbc0199627b&t=VGlrVG9rIFNvbmdz&tc=000000&vkey=1


#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Read<T> {
    value: T,
    bytes: core::num::NonZeroUsize
}


// https://is1-ssl.mzstatic.com/image/thumb/AMCArtistImages126/v4/94/06/4d/94064d6b-c650-84a8-ae0a-bd3cf427898e/be14d48b-0f96-45d5-b15e-d255e87c48b6_ami-identity-795f9bb1320daa20b961333f6f8c6511-2023-08-17T07-24-42.519Z_cropped.png/600x600cc.jpg
// https://is1-ssl.mzstatic.com/image/thumb/Features125/v4/8c/2d/b0/8c2db00d-c4e0-792c-7978-a87a6097225e/mzl.umyvofta.jpg/600x600SC.FPESS03.jpg
// https://is1-ssl.mzstatic.com/image/thumb/AMCArtistImages112/v4/c0/53/ab/c053ab54-b607-cb7a-89d0-23750f3a906a/d6059c29-5102-4162-9250-ca101aeeb3bd_ami-identity-60e298455bd895221b74629334666db7-2022-10-25T12-13-18.445Z_cropped.png/600x600SC.FPESS03.jpg
// https://is1-ssl.mzstatic.com/image/thumb/Features/v4/fd/9f/6c/fd9f6cd2-ba7b-4fcd-8f5b-309329f7d5e6/edd8c7ef-bdf9-4065-916f-9fb5527b5fe4.png/600x600SC.CAHGOY01.jpg
// https://is1-ssl.mzstatic.com/image/thumb/Features116/v4/e3/6e/33/e36e33de-da6c-71e8-dc2e-fae168c924ba/47a3c06a-ab88-4d36-a040-41d730e71d45.png/600x600cc.jpg

// https://is1-ssl.mzstatic.com/image/thumb/Music221/v4/47/98/ae/4798ae9f-3199-dffa-980c-1d7c9ba56189/artwork.jpg/520x520ac.jpg
// https://is1-ssl.mzstatic.com/image/thumb/AMCArtistImages211/v4/a6/fc/cc/a6fcccca-d0e5-884e-f20b-fc69885c150a/0360f9e4-6080-4161-992f-fe6195c8c1a3_file_cropped.png/520x520bb.jpg


// http://is5.mzstatic.com/image/pf/us/r30/Music221/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.jpg
// => "No source image found 'Music221/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.lsr' from ARRepoMan"
//     ???? lsr ?? why 
// http://is5.mzstatic.com/image/pf/us/r30/Music/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.lsr
// => "No source image found 'Music/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.JPEG' from ARRepoMan"
//     ?????? JPEG??????
// ok is it resolving correctly for image but image needs LSR for gen ??? or vise versa ?? idfk




// oh fuck

// https://is1-ssl.mzstatic.com/image/thumb/gen/600x600AM.PDCXS01.jpg?c1=FFFFFF&c2=CCA3A3&c3=960019&c4=1A1414&signature=cd00baed652789cfa36f326160fcf46c7786df4366fd6f2fbd189bbc0199627b&t=VGlrVG9rIFNvbmdz&tc=000000&vkey=1
// permits reordering
// denies excess
// can't remove extraneous zeros from #000000
// allows duplicate but only first one matters

// in artwork db had token "rt.1727187618"









// CDN or proxy is "daiquiri/5" (or previously "ATS/4.1.0"?)



// https://is2-ssl.mzstatic.com/image/thumb/Music/0a/1f/85/mzi.adskaamt.tif/600x600bb.jpg
// https://is1-ssl.mzstatic.com/image/thumb/dGLqT-f-3AHL34XWpgSf-Q/256x256bb.jpg



// https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json
// what the fuck is this
// not related to apple but but HUHHH
// this is background drone footage for the apple tv ?? for autumn??



// https://a3.mzstatic.com/us/r30/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg => OK; implies before pool = not uuid?
// can turn into https://is1-ssl.mzstatic.com/image/thumb/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png
// can turn into https://is1-ssl.mzstatic.com/image/pf/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png


// http://is5.mzstatic.com/image/pf/us/r30/Music4/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.jpg
// -  u can change the r30 into whatever
// - can drop the region (the "autumn resources" shit is much less lenient seemingly)



// https://is1-ssl.mzstatic.com/image/thumb/oEYYIjc6-3zT0jgpyUiIaw/1x40at.png
// apple tv logo .. dawg ... is that a specific shit just for that
// i dont understand the resolution parameter on there

// https://a1.mzstatic.com/us/r1000/000/Features/oct_2012_event/LIVE/en/2.xml
// https://s.mzstatic.com/WebObjects/MZStoreServices.woa/wa/wsSearch?term=jack+johnson&limit=1 What




// i hate my life
// http://r.mzstatic.com/images/html_resources/da-storefront/tab_bar/update-xx.pdf
// https://s.mzstatic.com/images/html_resources/da-storefront/tab_bar_modern/top.pdf
// https://apps.mzstatic.com/content/static-config/android/manifest.json
// https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json
// https://apps.mzstatic.com/content/54a1317a0ad442d3965d64ef6bfaae1c/ there are fucking weird builds of apple music
//https://itc.mzstatic.com/ ??????/




// https://is1-ssl.mzstatic.com/image/thumb/BwzocRAAEgP6sUF5tTNW9w/492x492ve.webp
// ve ??? => it's on cast & crew on apple tv





// https://is5-ssl.mzstatic.com/image/thumb/Video116/v4/bb/87/22/bb87226e-0207-7574-cb38-671dbde126c3/pr_source.lsr/3840x2160.jpg
// https://is2-ssl.mzstatic.com/image/thumb/Podcasts112/v4/e7/a1/3c/e7a13c39-bf73-774f-174e-53b0edd4baad/mza_13783091193329431505.jpeg/1200x630wp.png
// https://is4-ssl.mzstatic.com/image/thumb/Purple/v4/ca/e4/3d/cae43d49-1e7d-62df-b4bd-f04f9783fc6d/mzl.drmitlev.png/750x750bb.jpeg
// Purple = games
// https://a5.mzstatic.com/us/r1000/0/Music122/v4/c8/03/57/c803571e-6d17-f10f-fddf-fd4f7fc00d5e/22UMGIM37441.rgb.jpg
// us? r1000? 0?

// us is some sort of region code
// can use "eu" https://a5.mzstatic.com/eu/r1000/0/Music122/v4/c8/03/57/c803571e-6d17-f10f-fddf-fd4f7fc00d5e/22UMGIM37441.rgb.jpg
// last part (the 0) does nothing ?? Tried changing to 1 and 3241234, same shasum
// - not here tho https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json it gotta be three zeros




// https://is1-ssl.mzstatic.com/image/thumb/Publication113/v4/7f/c5/a0/7fc5a0ee-a55b-4319-4741-c7e8dd669333/9781501194313.jpg/1200x630wz.png
// Publication
// https://is5-ssl.mzstatic.com/image/thumb/Features123/v4/01/ce/2d/01ce2d37-f9e4-fc0a-3145-1ab88490c00d/source/1800x1800cc.jpg

// https://is1-ssl.mzstatic.com/image/thumb/PurpleSource211/v4/79/ca/8a/79ca8aec-2c59-83d4-065d-f0b19aa57880/c57609fe-03c2-4064-9633-24ce513d46e5_0x0ss.png/626x0w.webp
// https://is1-ssl.mzstatic.com/image/thumb/Purple221/v4/bb/9a/1b/bb9a1b9c-3727-2e06-ae5a-6cf5ec4fd8f4/pr_source.png/626x0w.webp


// https://is1-ssl.mzstatic.com/image/thumb/WkLx7oCZ0vBL7G2rzdkcbQ/626x392sr.webp
// ^ apple "happening now"

// fuck it doesnt have a pool ima kms






// https://is1-ssl.mzstatic.com/image/thumb/WkLx7oCZ0vBL7G2rzdkcbQ/0x300h.webp // infer width by resolution
// https://is1-ssl.mzstatic.com/image/thumb/WkLx7oCZ0vBL7G2rzdkcbQ/300x0w.webp // infer height by resolution

// https://a3.mzstatic.com/us/r1000/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg => invalid token {uuid}
// https://a3.mzstatic.com/us/r30/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg => OK; implies before pool = not uuid?
// can turn into https://is1-ssl.mzstatic.com/image/thumb/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png
// can turn into https://is1-ssl.mzstatic.com/image/pf/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg/300x300.png


// http://is5.mzstatic.com/image/pf/us/r30/Music4/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.jpg
// WHAT THE FUCK IS THE "PF" IN THERE???
// can *not* turn it into a "thumb" despite earlier one going thumb => pf

// dawg u can just
// remove it ??
// http://is5.mzstatic.com/image/pf/Music4/v4/a1/61/b1/a161b1f0-4882-82f5-017b-da8f9c8aea49/0724352141050_1500x1500_300dpi.1200x1200-75.jpg


// https://a3.mzstatic.com/us/r30/Video/v4/3e/8a/b0/3e8ab028-45f5-a0dd-6a6f-84daf9f93a11/101_Dalmatians_E-Distribution_Standard_Electronic_Apple_Taiwan.jpg


// https://is1-ssl.mzstatic.com/image/thumb/Purple69/v4/dc/2d/0e/dc2d0e06-3aff-b319-fdd2-d3bc33852bf6/pr_source.png/0x0ss.jpg
// 0x0 ss ?????



// /screen322x572.jpeg
// /poster300x300.jpeg
// ^ invalid now?

// https://a3.mzstatic.com/eu/r30/Purple6/v4/35/b0/30/35b03001-bd55-be5d-ece6-491356bc90b6/screen568x568.jpeg
//                         ^^ eu, saw US earlier, must be region
// but what is the other ?? and the zero i saw
// https://a1.mzstatic.com/us/r1000/049/Music/6e/d3/a2/mzi.qyqsqoep.1200x1200-75.jpg
// like whats that 049


// https://a3.mzstatic.com/us/r30/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg old & broke
// https://a3.mzstatic.com/us/r30/Publication/v4/fb/a1/05/fba10510-2240-72e0-979c-ae2e0e1cb925/cover.225x225-75.jpg old(12yr) & broke

