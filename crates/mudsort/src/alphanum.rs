//! mudsort's string order (`AlphanumComparator.cs`, Dave Koelle's
//! "alphanum" algorithm): each string is cut into runs of digits and runs of
//! everything else; two digit runs compare as numbers, anything else as
//! text; the first run that differs decides.
//!
//! The text comparison is .NET's `String.CompareTo`, which compares in the
//! player's culture. mudsort ran under .NET Framework on Windows, where that
//! is the system's language order; [`compare_text`] reproduces the en-US
//! order (the golden test checks it against .NET Framework itself).

use std::cmp::Ordering;

/// Compares two strings as mudsort's `AlphanumComparator.Compare` does.
#[must_use]
pub fn compare(s1: &str, s2: &str) -> Ordering {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let mut this_marker = 0;
    let mut that_marker = 0;
    while this_marker < a.len() || that_marker < b.len() {
        if this_marker >= a.len() {
            return Ordering::Less;
        } else if that_marker >= b.len() {
            return Ordering::Greater;
        }
        let this_chunk = chunk(&a, &mut this_marker);
        let that_chunk = chunk(&b, &mut that_marker);
        let result = if is_digit(this_chunk[0]) && is_digit(that_chunk[0]) {
            compare_numbers(this_chunk, that_chunk)
        } else {
            compare_text_chars(this_chunk, that_chunk)
        };
        if result != Ordering::Equal {
            return result;
        }
    }
    Ordering::Equal
}

/// A digit, as the comparator splits runs. .NET's `char.IsDigit` also
/// takes other scripts' digits, which `Convert.ToInt64` then refuses (the
/// sort stopped with an error); only ASCII digits are digits here.
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// The run starting at `marker`: digits, or everything up to the next digit.
fn chunk<'a>(s: &'a [char], marker: &mut usize) -> &'a [char] {
    let start = *marker;
    let numeric = is_digit(s[start]);
    *marker += 1;
    while *marker < s.len() && is_digit(s[*marker]) == numeric {
        *marker += 1;
    }
    &s[start..*marker]
}

/// Two digit runs by value, as `Convert.ToInt64` reads them ("01" equals
/// "1"). A run longer than a 64-bit number made mudsort's sort fail; here it
/// compares by value too.
fn compare_numbers(a: &[char], b: &[char]) -> Ordering {
    let trim = |s: &'_ [char]| -> usize { s.iter().take_while(|c| **c == '0').count() };
    let a = &a[trim(a)..];
    let b = &b[trim(b)..];
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// Compares two strings as .NET Framework's `String.CompareTo` does under
/// en-US.
#[must_use]
pub fn compare_text(a: &str, b: &str) -> Ordering {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    compare_text_chars(&a, &b)
}

/// Windows' culture order, by levels: first the characters' base order,
/// ignoring case, accents, apostrophes and hyphens; then accents; then case
/// (lower case first); then where the apostrophes and hyphens are (none
/// first). Symbols come before digits, digits before letters.
fn compare_text_chars(a: &[char], b: &[char]) -> Ordering {
    let a = SortKey::of(a);
    let b = SortKey::of(b);
    a.primary
        .cmp(&b.primary)
        .then_with(|| a.accent.cmp(&b.accent))
        .then_with(|| a.case.cmp(&b.case))
        .then_with(|| a.special.cmp(&b.special))
}

/// One string's weights, level by level.
#[derive(Debug, Default)]
struct SortKey {
    primary: Vec<u32>,
    accent: Vec<u32>,
    case: Vec<u8>,
    special: Vec<(usize, u8)>,
}

/// The symbols in their culture order (after the apostrophe and the hyphen,
/// which only count last).
const SYMBOLS: &str = " !\"#$%&()*,./:;?@[\\]^_`{|}~+<=>";

const DIGIT_BASE: u32 = 100;
const LETTER_BASE: u32 = 200;
const OTHER_BASE: u32 = 1000;

impl SortKey {
    fn of(s: &[char]) -> Self {
        let mut key = Self::default();
        for (index, &c) in s.iter().enumerate() {
            match c {
                '\'' => key.special.push((index, 1)),
                '-' => key.special.push((index, 2)),
                // The ligature counts as the two letters, at every level.
                'æ' | 'Æ' => {
                    let upper = c == 'Æ';
                    for letter in ['a', 'e'] {
                        key.push(LETTER_BASE + (letter as u32 - 'a' as u32), 0, upper);
                    }
                }
                _ => {
                    let (primary, accent, upper) = weigh(c);
                    key.push(primary, accent, upper);
                }
            }
        }
        key
    }

    fn push(&mut self, primary: u32, accent: u32, upper: bool) {
        self.primary.push(primary);
        self.accent.push(accent);
        self.case.push(u8::from(upper));
    }
}

/// A character's base weight, accent weight and case.
fn weigh(c: char) -> (u32, u32, bool) {
    if let Some(position) = SYMBOLS.find(c) {
        return (1 + position as u32, 0, false);
    }
    if c.is_ascii_digit() {
        return (DIGIT_BASE + (c as u32 - '0' as u32), 0, false);
    }
    if c.is_ascii_alphabetic() {
        let lower = c.to_ascii_lowercase();
        return (
            LETTER_BASE + (lower as u32 - 'a' as u32),
            0,
            c.is_ascii_uppercase(),
        );
    }
    let upper = c.is_uppercase();
    let lower = c.to_lowercase().next().unwrap_or(c);
    if let Some((base, accent)) = latin1_letter(lower) {
        return (LETTER_BASE + (base as u32 - 'a' as u32), accent, upper);
    }
    (OTHER_BASE + c as u32, 0, false)
}

/// A lower-case Latin-1 letter with a mark: its base letter, and the mark's
/// rank in the culture order (acute, grave, circumflex, diaeresis, tilde,
/// ring, stroke; the cedilla is the only mark on a c).
fn latin1_letter(c: char) -> Option<(char, u32)> {
    Some(match c {
        'á' => ('a', 1),
        'à' => ('a', 2),
        'â' => ('a', 3),
        'ä' => ('a', 4),
        'ã' => ('a', 5),
        'å' => ('a', 6),
        'ç' => ('c', 8),
        'é' => ('e', 1),
        'è' => ('e', 2),
        'ê' => ('e', 3),
        'ë' => ('e', 4),
        'í' => ('i', 1),
        'ì' => ('i', 2),
        'î' => ('i', 3),
        'ï' => ('i', 4),
        'ñ' => ('n', 5),
        'ó' => ('o', 1),
        'ò' => ('o', 2),
        'ô' => ('o', 3),
        'ö' => ('o', 4),
        'õ' => ('o', 5),
        'ø' => ('o', 7),
        'ú' => ('u', 1),
        'ù' => ('u', 2),
        'û' => ('u', 3),
        'ü' => ('u', 4),
        'ý' => ('y', 1),
        'ÿ' => ('y', 4),
        _ => return None,
    })
}

/// Sorts `list` as .NET Framework's `ArrayList.Sort(IComparer)` does for a
/// program without a target framework (a native host such as the game
/// client): a depth-limited quicksort with a median-of-three pivot, heap
/// sort past 32 levels. It is not stable: values the comparer calls equal
/// ("1" and "01") end in the order this algorithm leaves them, which the
/// golden test pins.
pub fn dotnet_sort<T>(list: &mut [T], compare: impl Fn(&T, &T) -> Ordering) {
    if list.len() > 1 {
        let right = list.len() - 1;
        depth_limited_quick_sort(list, &compare, 0, right, 32);
    }
}

fn swap_if_greater<T>(list: &mut [T], compare: &impl Fn(&T, &T) -> Ordering, a: usize, b: usize) {
    if a != b && compare(&list[a], &list[b]) == Ordering::Greater {
        list.swap(a, b);
    }
}

fn depth_limited_quick_sort<T>(
    list: &mut [T],
    compare: &impl Fn(&T, &T) -> Ordering,
    mut left: usize,
    mut right: usize,
    mut depth_limit: u32,
) {
    loop {
        if depth_limit == 0 {
            heap_sort(list, compare, left, right);
            return;
        }
        // Signed, as in .NET: `j` may step below `left`.
        let mut i = left as isize;
        let mut j = right as isize;
        let middle = left + ((right - left) >> 1);
        swap_if_greater(list, compare, left, middle);
        swap_if_greater(list, compare, left, right);
        swap_if_greater(list, compare, middle, right);
        let x = middle;
        // The pivot moves when it is swapped; follow it by position as .NET
        // follows the object it copied out.
        let mut pivot = x;
        loop {
            while compare(&list[i as usize], &list[pivot]) == Ordering::Less {
                i += 1;
            }
            while compare(&list[pivot], &list[j as usize]) == Ordering::Less {
                j -= 1;
            }
            if i > j {
                break;
            }
            if i < j {
                list.swap(i as usize, j as usize);
                if pivot == i as usize {
                    pivot = j as usize;
                } else if pivot == j as usize {
                    pivot = i as usize;
                }
            }
            i += 1;
            j -= 1;
            if i > j {
                break;
            }
        }
        depth_limit -= 1;
        if j - left as isize <= right as isize - i {
            if (left as isize) < j {
                depth_limited_quick_sort(list, compare, left, j as usize, depth_limit);
            }
            left = i as usize;
        } else {
            if i < right as isize {
                depth_limited_quick_sort(list, compare, i as usize, right, depth_limit);
            }
            right = j as usize;
        }
        if left >= right {
            return;
        }
    }
}

fn heap_sort<T>(list: &mut [T], compare: &impl Fn(&T, &T) -> Ordering, lo: usize, hi: usize) {
    let n = hi - lo + 1;
    let mut i = n / 2;
    while i >= 1 {
        down_heap(list, compare, i, n, lo);
        i -= 1;
    }
    let mut i = n;
    while i > 1 {
        list.swap(lo, lo + i - 1);
        down_heap(list, compare, 1, i - 1, lo);
        i -= 1;
    }
}

fn down_heap<T>(
    list: &mut [T],
    compare: &impl Fn(&T, &T) -> Ordering,
    mut i: usize,
    n: usize,
    lo: usize,
) {
    // .NET lifts the element out and shifts children up; swapping it down
    // step by step ends with every element in the same place.
    while i <= n / 2 {
        let mut child = 2 * i;
        if child < n && compare(&list[lo + child - 1], &list[lo + child]) == Ordering::Less {
            child += 1;
        }
        if compare(&list[lo + i - 1], &list[lo + child - 1]) != Ordering::Less {
            break;
        }
        list.swap(lo + i - 1, lo + child - 1);
        i = child;
    }
}
