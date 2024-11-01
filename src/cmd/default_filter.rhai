/*
Hi!

This is the filter file, in here you can write Rhai code to filter out tracks 
during the sync process.

This file must contain the `filter(track)` function, which will be evaluated for 
every candidate copy track: return `true` to copy the track.

If you want to match any String to a regex, call `regex_match(pattern, your_string)`.

The `track` object that's being passed as argument is the same as `model::BaseTrack`.
*/

fn filter(track) {
	// add your filtering code here!

	true
}
