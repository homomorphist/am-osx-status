// ObjC.import("CoreServices");

// // // function cf_to_ns(cf) {
// // //     const ns = $.NSArray.alloc;
// // //     for (var i = 0, len = $.CFArrayGetCount(cf); i < len; i++) {
// // //         ns.addObject($.CFArrayGetValueAtIndex(cf, i))
// // //     }
// // //     return ns


        
// // // // }

// // function read_cf_str(ref) {
// //     return $.CFDataGetBytePtr($.CFStringCreateExternalRepresentation(null, ref, "UTF-8", 0))
// // }

// // function CFTypeName(ref) {
// //     const id = $.CFGetTypeID(ref)
// //     const desc = $.CFCopyTypeIDDescription(id);
// //     return read_cf_str(desc)
// // }

// // function registered(id) {
// //     const nil = $()
// //     const CFArray = $.LSCopyApplicationURLsForBundleIdentifier(id, nil);

// //     console.log(CFTypeName(CFArray))
// //     // for (const k in CFArray[0][0]) {
// //         // console.log(k)
// //     // }
// //     const v = CFArray[0]
// //     console.log(v)
// //     console.log(CFTypeName(v))

// //     // cf_to_ns(CFArray)
// //     const NSArray = $.NSArray.arrayWithArray((CFArray)).valueForKey("path")
// //     // return ObjC.deepUnwrap(NSArray);
// //     // // return urls && urls.count() > 0;
// // }

// // // function isBundlePresentViaMDFind(bundleId) {
// // //     const app = Application.currentApplication();
// // //     app.includeStandardAdditions = true;

// // //     console.log("1")
// // //     const result = app.doShellScript(`mdfind "kMDItemCFBundleIdentifier == '${bundleId}'"`);
// // //     console.log("2")
// // //     return result.trim().length > 0;
// // // }



// // console.log()
// // console.log()
// // console.log("---- Music:")
// // registered("com.apple.Music")
// // console.log()
// // console.log("---- iTunes:")
// // registered("com.apple.iTunes")
// // console.log()
// // console.log()
// // // console.log("Music:", isBundlePresentViaMDFind("com.apple.Music"))
// // // console.log("Music:", isBundlePresentViaMDFind("com.apple.iTunes"))



// // ```
// // ObjC.import('AppKit');

// // function appExists(bundleId) {
// //     const ws = $.NSWorkspace.sharedWorkspace;
// //     const apps = ws.runningApplications.js;
// //     return apps.some(app => app.bundleIdentifier.js === bundleId);
// // }
// // ```


// // ```
// // /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -dump
// // ```


// // ```
// // ObjC.import("AppKit");

// // function findAppPathByBundleId(targetId) {
// //     const ws = $.NSWorkspace.sharedWorkspace;
// //     const apps = ws.runningApplications.js;
// //     for (const app of apps) {
// //         if (app.bundleIdentifier.js === targetId) {
// //             return app.bundleURL.path.js;
// //         }
// //     }
// //     return null;
// // }

// // console.log("Music running at:", findAppPathByBundleId("com.apple.Music"));
// // console.log("iTunes running at:", findAppPathByBundleId("com.apple.iTunes"));
// // ```



const player = Application("com.apple.Music");


console.log(player.exists())