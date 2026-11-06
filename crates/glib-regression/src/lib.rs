//! Synthetic regressions for the workspace's GLib 0.18.5 backport.
//!
//! The tests call the real GLib FFI through `Variant::array_iter_str`. They use
//! only in-memory fixtures: no AgentDeck configuration, agents, network access,
//! external processes, or desktop initialization. Non-Linux builds contain no
//! tests and have no GLib dependency.

#[cfg(all(test, target_os = "linux"))]
mod tests {
    // These exact entry points are the subject of the regression tests.
    #![allow(clippy::double_ended_iterator_last, clippy::iter_nth_zero)]

    use glib::{Variant, VariantStrIter, variant::ToVariant};

    const STRINGS: [&str; 6] = ["ascii", "", "café", "中文", "🦀", "tail"];
    const CONTENT_CASES: [&str; 5] = ["ascii", "", "café", "中文", "🦀"];

    fn strings(values: &[&str]) -> Variant {
        let variant = values.to_variant();
        assert_eq!(variant.type_().as_str(), "as");
        variant
    }

    fn assert_remaining(iter: &VariantStrIter<'_>, expected: usize) {
        assert_eq!(iter.len(), expected);
        assert_eq!(iter.size_hint(), (expected, Some(expected)));
    }

    fn assert_exhausted(iter: &mut VariantStrIter<'_>) {
        assert_remaining(iter, 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        assert_eq!(iter.nth(0), None);
        assert_eq!(iter.nth_back(0), None);
        assert_eq!(iter.nth(usize::MAX), None);
        assert_eq!(iter.nth_back(usize::MAX), None);
        assert_remaining(iter, 0);
    }

    #[test]
    fn next_reads_ascii_empty_and_unicode() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        for (index, expected) in STRINGS.iter().enumerate() {
            assert_remaining(&iter, STRINGS.len() - index);
            assert_eq!(iter.next(), Some(*expected));
        }
        assert_exhausted(&mut iter);
    }

    #[test]
    fn nth_reads_ascii_empty_and_unicode() {
        for expected in CONTENT_CASES {
            let variant = strings(&["skip", expected, "tail"]);
            let mut iter = variant.array_iter_str().unwrap();
            assert_eq!(iter.nth(1), Some(expected));
            assert_remaining(&iter, 1);
            assert_eq!(iter.next(), Some("tail"));
            assert_exhausted(&mut iter);
        }
    }

    #[test]
    fn last_reads_ascii_empty_and_unicode() {
        for expected in CONTENT_CASES {
            let variant = strings(&["prefix", expected]);
            let iter = variant.array_iter_str().unwrap();
            // Consume VariantStrIter itself, so this reaches its last override.
            assert_eq!(iter.last(), Some(expected));
        }
    }

    #[test]
    fn next_back_reads_ascii_empty_and_unicode() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        for (index, expected) in STRINGS.iter().rev().enumerate() {
            assert_remaining(&iter, STRINGS.len() - index);
            assert_eq!(iter.next_back(), Some(*expected));
        }
        assert_exhausted(&mut iter);
    }

    #[test]
    fn nth_back_reads_ascii_empty_and_unicode() {
        for expected in CONTENT_CASES {
            let variant = strings(&["head", expected, "skip"]);
            let mut iter = variant.array_iter_str().unwrap();
            assert_eq!(iter.nth_back(1), Some(expected));
            assert_remaining(&iter, 1);
            assert_eq!(iter.next_back(), Some("head"));
            assert_exhausted(&mut iter);
        }
    }

    #[test]
    fn empty_array_supports_all_iterator_entry_points() {
        let variant = strings(&[]);
        let mut iter = variant.array_iter_str().unwrap();
        assert_exhausted(&mut iter);
        assert_eq!(variant.array_iter_str().unwrap().last(), None);
        assert_eq!(variant.array_iter_str().unwrap().count(), 0);
    }

    #[test]
    fn singleton_empty_string_is_not_an_empty_array() {
        let variant = strings(&[""]);
        assert_remaining(&variant.array_iter_str().unwrap(), 1);
        assert_eq!(variant.array_iter_str().unwrap().next(), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().nth(0), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().last(), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().next_back(), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().nth_back(0), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().count(), 1);
    }

    #[test]
    fn nth_out_of_bounds_exhausts_both_ends() {
        let variant = strings(&STRINGS);
        for index in [STRINGS.len(), STRINGS.len() + 1] {
            let mut iter = variant.array_iter_str().unwrap();
            assert_eq!(iter.nth(index), None);
            assert_exhausted(&mut iter);
        }
    }

    #[test]
    fn nth_back_out_of_bounds_exhausts_both_ends() {
        let variant = strings(&STRINGS);
        for index in [STRINGS.len(), STRINGS.len() + 1] {
            let mut iter = variant.array_iter_str().unwrap();
            assert_eq!(iter.nth_back(index), None);
            assert_exhausted(&mut iter);
        }
    }

    #[test]
    fn nth_usize_max_handles_nonzero_head_without_overflow() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some("ascii"));
        assert_eq!(iter.nth(usize::MAX), None);
        assert_exhausted(&mut iter);
    }

    #[test]
    fn nth_back_usize_max_handles_underflow() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next_back(), Some("tail"));
        assert_eq!(iter.nth_back(usize::MAX), None);
        assert_exhausted(&mut iter);
    }

    #[test]
    fn alternating_ends_visit_each_string_once() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some("ascii"));
        assert_eq!(iter.next_back(), Some("tail"));
        assert_eq!(iter.next(), Some(""));
        assert_eq!(iter.next_back(), Some("🦀"));
        assert_eq!(iter.next(), Some("café"));
        assert_eq!(iter.next_back(), Some("中文"));
        assert_exhausted(&mut iter);
    }

    #[test]
    fn mixed_skips_do_not_cross_the_remaining_range() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some("ascii"));
        assert_eq!(iter.next_back(), Some("tail"));
        assert_eq!(iter.nth(1), Some("café"));
        assert_remaining(&iter, 2);
        assert_eq!(iter.nth_back(1), Some("中文"));
        assert_exhausted(&mut iter);
    }

    #[test]
    fn last_respects_front_and_back_consumption() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.nth(1), Some(""));
        assert_eq!(iter.nth_back(1), Some("🦀"));
        assert_remaining(&iter, 2);
        assert_eq!(iter.last(), Some("中文"));
    }

    #[test]
    fn last_returns_none_after_exhaustion() {
        let variant = strings(&[""]);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next_back(), Some(""));
        assert_exhausted(&mut iter);
        assert_eq!(iter.last(), None);
    }

    #[test]
    fn len_size_hint_and_count_track_remaining_elements() {
        let variant = strings(&STRINGS);
        let mut iter = variant.array_iter_str().unwrap();
        assert_remaining(&iter, 6);
        assert_eq!(iter.next(), Some("ascii"));
        assert_remaining(&iter, 5);
        assert_eq!(iter.next_back(), Some("tail"));
        assert_remaining(&iter, 4);
        assert_eq!(iter.nth(1), Some("café"));
        assert_remaining(&iter, 2);
        assert_eq!(iter.count(), 2);
        assert_eq!(variant.array_iter_str().unwrap().count(), STRINGS.len());
    }

    #[test]
    fn count_is_zero_after_out_of_bounds_consumption() {
        let variant = strings(&STRINGS);
        let mut forward = variant.array_iter_str().unwrap();
        assert_eq!(forward.nth(usize::MAX), None);
        assert_eq!(forward.count(), 0);
        let mut backward = variant.array_iter_str().unwrap();
        assert_eq!(backward.nth_back(usize::MAX), None);
        assert_eq!(backward.count(), 0);
    }

    #[test]
    fn collected_forward_and_reverse_values_match_the_fixture() {
        let variant = strings(&STRINGS);
        assert_eq!(
            variant.array_iter_str().unwrap().collect::<Vec<_>>(),
            STRINGS
        );
        assert_eq!(
            variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
            STRINGS.iter().rev().copied().collect::<Vec<_>>()
        );
    }
}
