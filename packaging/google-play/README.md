# Google Play

dnt on Google Play is an app store's build (`DNT_STORE=1`, see
[`app/README.md`](../../app/README.md#app-stores)): Play updates it, so it
has no updater and no permission to install apps, which Play's policies
forbid outside the store.

## The bundle

Each release has `dnt-app-android-play.aab`, the Android App Bundle Play
takes (built by [`app.yml`](../../.github/workflows/app.yml)). It is
signed with the release key (`ANDROID_KEYSTORE`), which for Play is the
**upload key**: Play checks uploads with it and signs what it installs
with its own key (Play App Signing). So an install from Play, one from
GitHub and one from F-Droid cannot update each other.

To upload a release: download the `.aab` from the GitHub release, then in
the Play Console, Test and release › (a track) › Create new release.

## Setting it up, once

In the [Play Console](https://play.google.com/console):

1. **Create app**: name *Dungeons & Terminals*, an app (not a game), free.
2. **Play App Signing**: let Google make and keep the app signing key.
   On the first upload, the bundle's key becomes the upload key.
3. **Store listing**: from `fastlane/metadata/android/en-US/`:
   `title.txt`, `short_description.txt`, `full_description.txt` (Play
   takes its HTML tags), `images/icon.png`, `images/featureGraphic.png`
   and `images/phoneScreenshots/`. Category: Entertainment (or Tools).
4. **Privacy policy**:
   `https://github.com/v0v3s/dungeons-n-terminals/blob/main/PRIVACY.md`.
5. **App content** (Policy › App content), the answers for dnt:
   - *Ads*: no ads.
   - *App access*: all functionality is available without special
     access (no account, no login).
   - *Content rating* (the IARC questionnaire): category *Utility,
     Productivity, Communication, or Other*; no violence shown (the rules
     describe combat in words), no user-to-user communication, no sharing
     of location, no purchases, no gambling.
   - *Target audience*: 13 and over (not designed for children).
   - *Data safety*: the app collects no data and shares no data
     (everything stays on the device; the rules library is a download,
     with nothing about the user sent), so the form's further questions
     do not apply.
   - *Government app*, *financial features*, *health*, *news*: no.
6. **Testing**: a new personal account needs a **closed test** with at
   least 12 testers opted in for 14 days in a row before it can apply for
   production. Add their Google accounts (or a Google Group) to a closed
   track, upload the bundle there and share the opt-in link.
7. **Production**: after the closed test, apply for production access,
   then release the same bundle there.

## Keep in step

- The listing in the Play Console is not read from the repository: when
  `fastlane/metadata` changes, change it there too (or automate it later
  with a service account and `fastlane supply`).
- The bundle's version is the release's (`versionCode` in
  `app/src-tauri/tauri.conf.json`); Play refuses a code it has seen.
- Play requires a recent `targetSdk`
  (`app/src-tauri/gen/android/app/build.gradle.kts`), raised each year.
