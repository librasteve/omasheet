// Copyright (c) 2026 Stephen Roe

//! How a sheet is styled, written in Markdown.
//!
//! A column's alignment is the colons of its `| :--- | :---: | ---: |` line.
//! A header cell and the leftmost cell of a row are written as Markdown
//! too: `**bold**`, `_italic_` and `## heading`. A header cell styles its
//! whole column and a leftmost cell its whole row, so a style is written
//! once. The markers are not part of the name or the value.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    /// Not said: numbers to the right, anything else to the left.
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub align: Align,
    pub bold: bool,
    pub italic: bool,
    /// 1 to 6 for `#` to `######`; 0 for none.
    pub level: u8,
    /// `^` in front: a row (or column) that sums up the block above it
    /// rather than being one of its lines. See `outline`.
    pub summary: bool,
    /// A column's width in characters, as the dashes of its rule line
    /// say; 0 for none, which fits the column to its cells.
    pub width: u16,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        *self == Style::default()
    }

    /// Bold, italic and the heading level as one number, for a caller that
    /// holds styles as numbers: bit 0 bold, bit 1 italic, the level above.
    pub fn bits(&self) -> u32 {
        self.bold as u32
            | (self.italic as u32) << 1
            | (self.level as u32) << 2
            | (self.summary as u32) << 5
    }

    /// `l`, `c` or `r`; empty for none said.
    pub fn align_letter(&self) -> &'static str {
        match self.align {
            Align::Auto => "",
            Align::Left => "l",
            Align::Center => "c",
            Align::Right => "r",
        }
    }

    /// The alignment a `---` cell of the rule line says.
    pub fn align_of_rule(cell: &str) -> Align {
        let cell = cell.trim();
        match (cell.starts_with(':'), cell.ends_with(':') && cell.len() > 1) {
            (true, true) => Align::Center,
            (true, false) => Align::Left,
            (false, true) => Align::Right,
            (false, false) => Align::Auto,
        }
    }

    /// The `---` cell of the rule line, `width` characters wide.
    pub fn rule(&self, width: usize) -> String {
        let w = if self.width > 0 {
            self.width as usize
        } else {
            width
        }
        .max(3);
        match self.align {
            Align::Auto => "-".repeat(w),
            Align::Left => format!(":{}", "-".repeat(w - 1)),
            Align::Right => format!("{}:", "-".repeat(w - 1)),
            Align::Center => format!(":{}:", "-".repeat(w - 2)),
        }
    }

    /// The style a cell's text is written with, and the text without it.
    /// Text that is not wrapped in markers is plain, and is returned as it
    /// is. Quote a cell to keep its markers: `"**x**"`.
    pub fn strip(text: &str) -> (Style, &str) {
        let mut style = Style::default();
        let mut t = text;
        if let Some(rest) = t.strip_prefix("^ ")
            && !rest.trim().is_empty()
        {
            style.summary = true;
            t = rest.trim_start();
        }
        let hashes = t.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes)
            && t[hashes..].starts_with(' ')
            && !t[hashes..].trim().is_empty()
        {
            style.level = hashes as u8;
            t = t[hashes..].trim_start();
        }
        loop {
            let wrapped = |m: &str| {
                t.len() > 2 * m.len() && t.starts_with(m) && t.ends_with(m) && {
                    let inner = &t[m.len()..t.len() - m.len()];
                    !inner.trim_matches(['*', '_']).is_empty()
                        && (m.len() == 2 || !inner.starts_with(m) && !inner.ends_with(m))
                        && !inner.starts_with(char::is_whitespace)
                        && !inner.ends_with(char::is_whitespace)
                }
            };
            if wrapped("**") {
                style.bold = true;
                t = &t[2..t.len() - 2];
            } else if wrapped("_") || wrapped("*") {
                style.italic = true;
                t = &t[1..t.len() - 1];
            } else {
                break;
            }
        }
        (style, t)
    }

    /// `text` written with this style's markers.
    pub fn wrap(&self, text: &str) -> String {
        let mut out = match (self.bold, self.italic) {
            (true, true) => format!("**_{text}_**"),
            (true, false) => format!("**{text}**"),
            (false, true) => format!("_{text}_"),
            (false, false) => text.to_string(),
        };
        if (1..=6).contains(&self.level) {
            out = format!("{} {out}", "#".repeat(self.level as usize));
        }
        if self.summary {
            out = format!("^ {out}");
        }
        out
    }

    /// A column's style with a row's: bold and italic add up, and the
    /// column's heading level wins over the row's, which is used only
    /// where the column has none. The alignment is the column's alone.
    pub fn over(self, row: Style) -> Style {
        Style {
            align: self.align,
            bold: self.bold || row.bold,
            italic: self.italic || row.italic,
            level: if self.level > 0 {
                self.level
            } else {
                row.level
            },
            summary: self.summary || row.summary,
            width: 0,
        }
    }
}

/// A block of lines that a heading level declares: the head line, and
/// the lines after it up to the next line of its level or a higher one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub head: usize,
    /// One past the last line of the block.
    pub end: usize,
    /// How many blocks it is inside.
    pub depth: usize,
}

/// The blocks of a run of lines, and which block each summary line closes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outline {
    pub blocks: Vec<Block>,
    /// For each line, the head of the block it sums up, if it is a summary
    /// line (`^`) that has one.
    pub owner: Vec<Option<usize>>,
}

impl Outline {
    /// The lines of the block headed `head` that closing it hides: all
    /// but the summary lines of a block around it. Its own summary lines
    /// go too: the head shows their values while it is closed.
    pub fn hidden(&self, head: usize) -> impl Iterator<Item = usize> + '_ {
        let end = self
            .blocks
            .iter()
            .find(|b| b.head == head)
            .map_or(head + 1, |b| b.end);
        (head + 1..end).filter(move |&r| !matches!(self.owner[r], Some(o) if o < head))
    }
}

/// The blocks of lines whose heading levels are `levels`, 0 for a line
/// with none, and `summary` which lines are summary lines. The same for
/// rows and for columns. A head with nothing under it is only a heading
/// and makes no block.
///
/// A summary line is no head and ends no block, whatever its level. It
/// belongs to the innermost block it is inside, or, if it has a level, to
/// the innermost block headed by that level: so `^ ## Region` sums up the
/// `##` block it ends, and a plain `^ Total` the block closest to it.
pub fn outline(levels: &[u8], summary: &[bool]) -> Outline {
    let is_sum = |i: usize| summary.get(i).copied().unwrap_or(false);
    let level = |i: usize| if is_sum(i) { 0 } else { levels[i] };
    let mut blocks: Vec<Block> = Vec::new();
    // The blocks that contain the line being read, by where they end.
    let mut open: Vec<usize> = Vec::new();
    for i in 0..levels.len() {
        open.retain(|&end| end > i);
        if level(i) == 0 {
            continue;
        }
        let end = (i + 1..levels.len())
            .find(|&j| level(j) > 0 && level(j) <= level(i))
            .unwrap_or(levels.len());
        if end > i + 1 {
            blocks.push(Block {
                head: i,
                end,
                depth: open.len(),
            });
            open.push(end);
        }
    }
    let mut owner = vec![None; levels.len()];
    for r in (0..levels.len()).filter(|&r| is_sum(r)) {
        owner[r] = blocks
            .iter()
            .filter(|b| b.head < r && r < b.end && (levels[r] == 0 || level(b.head) == levels[r]))
            .next_back()
            .map(|b| b.head);
    }
    Outline { blocks, owner }
}

/// The blocks of `outline`, alone.
pub fn blocks(levels: &[u8], summary: &[bool]) -> Vec<Block> {
    outline(levels, summary).blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_and_wraps() {
        let (s, t) = Style::strip("## **_Qty_**");
        assert_eq!(t, "Qty");
        assert_eq!((s.level, s.bold, s.italic), (2, true, true));
        assert_eq!(s.wrap(t), "## **_Qty_**");
        assert_eq!(
            Style::strip("***x***").0,
            Style {
                bold: true,
                italic: true,
                ..Style::default()
            }
        );
        assert_eq!(Style::strip("*x*").1, "x");
    }

    #[test]
    fn leaves_plain_text_alone() {
        for t in [
            "*",
            "**",
            "***",
            "_",
            "a_b",
            "#1",
            "# ",
            "a * b * c",
            "\"**x**\"",
            "** x **",
        ] {
            assert_eq!(Style::strip(t), (Style::default(), t), "{t}");
        }
    }

    #[test]
    fn a_column_wins_over_a_row() {
        let col = Style {
            level: 2,
            ..Style::default()
        };
        let row = Style {
            level: 1,
            bold: true,
            ..Style::default()
        };
        let both = col.over(row);
        assert_eq!((both.level, both.bold), (2, true));
        assert_eq!(Style::default().over(row).level, 1);
    }

    #[test]
    fn levels_make_nested_blocks() {
        //            0  1  2  3  4  5  6
        let levels = [2, 0, 3, 0, 2, 0, 0];
        assert_eq!(
            blocks(&levels, &[]),
            [
                Block {
                    head: 0,
                    end: 4,
                    depth: 0
                },
                Block {
                    head: 2,
                    end: 4,
                    depth: 1
                },
                Block {
                    head: 4,
                    end: 7,
                    depth: 0
                },
            ]
        );
        // A heading with nothing under it is not a block.
        assert!(blocks(&[1, 1, 1], &[]).is_empty());
        assert!(blocks(&[0, 0], &[]).is_empty());
    }

    #[test]
    fn summaries_close_their_blocks() {
        //  0 ## A, 1 ### a1, 2 x, 3 ^ sub, 4 ### a2, 5 y, 6 ^ sub, 7 ^## A total, 8 ## B
        let levels = [2, 3, 0, 0, 3, 0, 0, 2, 2];
        let sum = [false, false, false, true, false, false, true, true, false];
        let o = outline(&levels, &sum);
        assert_eq!(
            o.blocks.iter().map(|b| (b.head, b.end)).collect::<Vec<_>>(),
            [(0, 8), (1, 4), (4, 8)]
        );
        assert_eq!(o.owner[3], Some(1));
        assert_eq!(o.owner[6], Some(4));
        assert_eq!(o.owner[7], Some(0));
        // Closing A hides all of it, its own total too; closing a1 hides its sub.
        assert_eq!(o.hidden(0).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(o.hidden(1).collect::<Vec<_>>(), [2, 3]);
        let (s, t) = Style::strip("^ ## **Total**");
        assert_eq!((s.summary, s.level, s.bold, t), (true, 2, true, "Total"));
        assert_eq!(s.wrap(t), "^ ## **Total**");
    }

    #[test]
    fn rules() {
        assert_eq!(Style::align_of_rule(":---"), Align::Left);
        assert_eq!(Style::align_of_rule(":---:"), Align::Center);
        assert_eq!(Style::align_of_rule("---:"), Align::Right);
        assert_eq!(Style::align_of_rule("---"), Align::Auto);
        let c = Style {
            align: Align::Center,
            ..Style::default()
        };
        assert_eq!(c.rule(5), ":---:");
    }
}
