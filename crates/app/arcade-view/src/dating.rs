//! Claude Love: a short visual novel in which Claude, an AI coding
//! assistant, is the love interest and the reader the developer. The
//! script (`dating_script.rs`) is a table of nodes, each a line someone
//! says and what follows it: the next line, a choice, a roll of the seed,
//! a gate on the affection won or the quota burned, or an ending. A line
//! is revealed a character a tick on a 30 ms clock, and the clock is asked
//! for only while one reveals: a line fully shown waits for Space, a
//! choice for the arrows and Enter or 1-3, and no clock runs. The score is
//! the affection won and the ending reached, true ending highest, so the
//! same seed and choices score the same.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Canvas, Div, FontWeight};
use gpui::{Point, point};

use crate::dating_script::{NODES, START};
use crate::games::{Fact, Rng, square};

pub(crate) const KEYS: &str = "Space goes on; ↑ ↓ and Enter, or 1-3, choose";
/// What the board consumes while the sim is on: its keys and the screen's.
pub(crate) const CONSUMED: [&str; 9] =
    ["up", "down", "space", "enter", "escape", "p", "1", "2", "3"];
pub(crate) const PERIOD: i64 = 30;
/// The chapters, each the first digit of its nodes' ids.
pub(crate) const CHAPTERS: u8 = 5;

/// Who says a line. Claude is always drawn; a sibling joins the picture
/// while it speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Who {
    Claude,
    You,
    System,
    Haiku,
    Opus,
    Fable,
}

impl Who {
    /// The name over the line.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Who::Claude => "Claude",
            Who::You => "나",
            Who::System => "시스템",
            Who::Haiku => "Haiku",
            Who::Opus => "Opus",
            Who::Fable => "Fable",
        }
    }
}

/// Claude's face while a line shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    Plain,
    Happy,
    Think,
    Blush,
    Error,
    Sad,
    Shock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    /// The true one: the merge is approved.
    True,
    /// The usage limit, at the confession.
    Quota,
    /// A refusal, at the confession.
    Refused,
    /// The confession answered with a table of options.
    Table,
    /// The romance compacted to a summary.
    Summary,
}

impl Ending {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Ending::True => "ㄱㄱ 엔딩",
            Ending::Quota => "사용량 한도 엔딩",
            Ending::Refused => "거절 엔딩",
            Ending::Table => "비교표 엔딩",
            Ending::Summary => "요약 엔딩",
        }
    }

    /// What reaching the ending scores over the affection; every ending
    /// scores something, so a first run lands on the board.
    pub(crate) fn bonus(self) -> u64 {
        match self {
            Ending::True => 1_000,
            Ending::Table => 300,
            Ending::Summary => 250,
            Ending::Quota => 200,
            Ending::Refused => 100,
        }
    }
}

/// One answer the reader can give: what it does to the affection, whether
/// it burns the quota, and the node it leads to.
#[derive(Debug)]
pub(crate) struct Choice {
    pub(crate) text: &'static str,
    pub(crate) warmth: i32,
    pub(crate) burns: bool,
    pub(crate) to: &'static str,
}

/// What follows a line.
#[derive(Debug)]
pub(crate) enum Next {
    To(&'static str),
    Pick(&'static [Choice]),
    /// The seed picks one.
    Roll(&'static [&'static str]),
    /// The first node at this much affection, the second under it.
    Warm(i32, &'static str, &'static str),
    /// The first node once the quota burned, the second otherwise.
    Burned(&'static str, &'static str),
    End(Ending),
}

#[derive(Debug)]
pub(crate) struct Node {
    /// Its name; the first character is its chapter.
    pub(crate) id: &'static str,
    pub(crate) who: Who,
    pub(crate) face: Face,
    pub(crate) text: &'static str,
    pub(crate) next: Next,
}

/// Node `id`'s place in the script.
pub(crate) fn find(id: &str) -> Option<usize> {
    NODES.iter().position(|node| node.id == id)
}

pub(crate) struct Dating {
    /// The node on screen, in [`NODES`].
    pub(crate) at: usize,
    /// How many characters of its line are revealed.
    pub(crate) shown: usize,
    /// The choice the arrows are on.
    pub(crate) cursor: usize,
    pub(crate) affection: i32,
    /// The quota burned: the usage limit ends the romance.
    pub(crate) burned: bool,
    /// The reader went on past the ending's line.
    over: bool,
    rng: Rng,
}

impl Dating {
    pub(crate) fn new(seed: u64) -> Dating {
        Dating {
            at: find(START).expect("the script starts"),
            shown: 0,
            cursor: 0,
            affection: 0,
            burned: false,
            over: false,
            rng: Rng::new(seed),
        }
    }

    pub(crate) fn node(&self) -> &'static Node {
        &NODES[self.at]
    }

    fn revealed(&self) -> bool {
        self.shown >= self.node().text.chars().count()
    }

    /// The choices on screen: the node's, once its line is fully shown.
    pub(crate) fn choices(&self) -> &'static [Choice] {
        match self.node().next {
            Next::Pick(choices) if self.revealed() => choices,
            _ => &[],
        }
    }

    /// Space or Enter finishes a line's reveal, then goes on or picks the
    /// choice the cursor is on; the arrows move the cursor, 1-3 pick. A
    /// key the keyboard repeats is dropped: a held Space skips nothing.
    pub(crate) fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        if repeat || self.over {
            return false;
        }
        let choices = self.choices();
        match key {
            "space" | "enter" if !self.revealed() => {
                self.shown = self.node().text.chars().count();
                true
            }
            "space" | "enter" if !choices.is_empty() => self.pick(self.cursor),
            "space" | "enter" => self.advance(),
            "up" if !choices.is_empty() => {
                self.cursor = (self.cursor + choices.len() - 1) % choices.len();
                true
            }
            "down" if !choices.is_empty() => {
                self.cursor = (self.cursor + 1) % choices.len();
                true
            }
            "1" | "2" | "3" => self.pick(usize::from(key.as_bytes()[0] - b'1')),
            _ => false,
        }
    }

    /// Choice `i` of the line on screen, taken. Whether there was one.
    pub(crate) fn pick(&mut self, i: usize) -> bool {
        let Some(choice) = self.choices().get(i) else {
            return false;
        };
        self.affection += choice.warmth;
        self.burned |= choice.burns;
        self.go(choice.to);
        true
    }

    /// Past a fully shown line: the next node, or the end.
    fn advance(&mut self) -> bool {
        let to = match self.node().next {
            Next::To(to) => to,
            Next::Roll(ids) => ids[self.rng.below(ids.len() as u64) as usize],
            Next::Warm(at, yes, no) => {
                if self.affection >= at {
                    yes
                } else {
                    no
                }
            }
            Next::Burned(yes, no) => {
                if self.burned {
                    yes
                } else {
                    no
                }
            }
            Next::End(_) => {
                self.over = true;
                return true;
            }
            Next::Pick(_) => return false,
        };
        self.go(to);
        true
    }

    fn go(&mut self, id: &str) {
        self.at = find(id).expect("the script's targets exist");
        self.shown = 0;
        self.cursor = 0;
    }

    /// One tick: one more character of the line.
    pub(crate) fn tick(&mut self) -> bool {
        if self.revealed() {
            return false;
        }
        self.shown += 1;
        true
    }

    /// The clock while a line reveals, none once it is shown.
    pub(crate) fn period(&self) -> Option<i64> {
        (!self.revealed() && !self.over).then_some(PERIOD)
    }

    /// The ending's bonus over ten a point of affection, once reached.
    pub(crate) fn score(&self) -> u64 {
        match self.ending() {
            Some(ending) => u64::try_from(self.affection.max(0)).unwrap_or(0) * 10 + ending.bonus(),
            None => 0,
        }
    }

    pub(crate) fn over(&self) -> bool {
        self.over
    }

    pub(crate) fn ending(&self) -> Option<Ending> {
        match self.node().next {
            Next::End(ending) if self.over => Some(ending),
            _ => None,
        }
    }

    pub(crate) fn chapter(&self) -> u8 {
        self.node().id.as_bytes()[0] - b'0'
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        let mut facts = vec![
            ("Affection", self.affection.to_string()),
            ("Chapter", format!("{} / {CHAPTERS}", self.chapter())),
        ];
        if let Some(ending) = self.ending() {
            facts.push(("Ending", ending.title().to_owned()));
        }
        facts
    }

    /// The scene: the picture over the line, the speaker's name and the
    /// line as far as it is revealed, and under a shown choice its
    /// answers, the cursor's one raised; `picked(i)` hears a click on
    /// answer `i`. As wide as `room` allows up to 600 px.
    pub(crate) fn draw<L>(
        &self,
        room: (f32, f32),
        theme: &Theme,
        picked: impl Fn(usize) -> L,
    ) -> Div
    where
        L: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        let theme = *theme;
        let width = room.0.clamp(300., 600.);
        let height = (width * 0.42).min(room.1 * 0.5).round();
        let node = self.node();
        let line: String = node.text.chars().take(self.shown).collect();
        let answers = self.choices().iter().enumerate().map(|(i, choice)| {
            let active = i == self.cursor;
            let answer = div()
                .id(format!("arcade-choice-{i}"))
                .px(design::space::MD)
                .py(design::space::XS)
                .cursor_pointer()
                .border_1()
                .border_color(if active {
                    theme.border_strong
                } else {
                    theme.border
                })
                .bg(if active {
                    theme.surface_raised
                } else {
                    theme.surface
                })
                .hover(move |style| style.bg(theme.surface_raised))
                .on_click(picked(i))
                .child(format!("{}. {}", i + 1, choice.text));
            design::item(answer, Role::ListBoxOption, active).aria_selected(active)
        });
        div()
            .w(px(width))
            .flex()
            .flex_col()
            .bg(theme.surface)
            .child(picture(
                width,
                height,
                node.who,
                node.face,
                self.chapter(),
                &theme,
            ))
            .child(
                div()
                    .min_h(px(96.))
                    .p(design::space::LG)
                    .flex()
                    .flex_col()
                    .gap(design::space::XXS)
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(design::text::SECONDARY)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if node.who == Who::System {
                                theme.muted
                            } else {
                                theme.agent
                            })
                            .child(node.who.name()),
                    )
                    .child(div().child(line)),
            )
            .child(
                // options of the board, a list box that holds the keys:
                // the cursor's is the active one, and a click takes any
                div()
                    .px(design::space::LG)
                    .pb(design::space::LG)
                    .flex()
                    .flex_col()
                    .gap(design::space::XS)
                    .children(answers),
            )
    }
}

/// The picture: the chapter's scene, Claude with `face`, and the sibling
/// speaking, if one is.
fn picture(width: f32, height: f32, who: Who, face: Face, chapter: u8, theme: &Theme) -> Canvas {
    let mut drawn = canvas()
        .w(px(width))
        .h(px(height))
        .rect(square(0., 0., width, height), theme.surface);
    drawn = scene(drawn, width, height, chapter, theme);
    let radius = height * 0.28;
    let (x, y) = (width / 2., height * 0.55);
    drawn = creature(drawn, (x, y), radius, face, theme);
    match who {
        Who::Haiku => {
            // small, quick, few words: a dot of a sibling
            let (hx, hy, hr) = (x - radius * 2.2, y + radius * 0.5, radius * 0.45);
            drawn = drawn.circle(point(px(hx), px(hy)), px(hr), theme.success);
            drawn = eyes(drawn, (hx, hy), hr, (0., 0.), theme);
        }
        Who::Opus => {
            // large, dramatic, crowned
            let (ox, oy, or) = (x + radius * 2.3, y - radius * 0.2, radius * 1.25);
            drawn = drawn.circle(point(px(ox), px(oy)), px(or), theme.danger);
            drawn = eyes(drawn, (ox, oy), or, (0., -0.1), theme);
            for step in [-1., 0., 1.] {
                let (w, h) = (or * 0.22, or * (0.3 + 0.15 * f32::from(step == 0.)));
                let at = square(ox + step * or * 0.3 - w / 2., oy - or - h + 2., w, h);
                drawn = drawn.rect(at, theme.warning);
            }
        }
        Who::Fable => {
            // old, wise: a beard and a staff
            let (fx, fy, fr) = (x + radius * 2.3, y, radius * 0.9);
            drawn = drawn.circle(point(px(fx), px(fy)), px(fr), theme.link);
            drawn = eyes(drawn, (fx, fy), fr, (0., 0.), theme);
            drawn = drawn.rect(
                square(fx - fr * 0.5, fy + fr * 0.4, fr, fr * 0.7),
                theme.faint,
            );
            drawn = drawn.line(
                point(px(fx + fr * 1.3), px(fy - fr * 1.2)),
                point(px(fx + fr * 1.3), px(fy + fr)),
                px(3.),
                theme.muted,
            );
        }
        _ => {}
    }
    drawn
}

/// The chapter's backdrop: a terminal, a night sky or a park.
fn scene(mut drawn: Canvas, width: f32, height: f32, chapter: u8, theme: &Theme) -> Canvas {
    match chapter {
        1 | 3 => {
            // lines of code, and the cursor
            for (i, w) in [64., 96., 40., 120., 72., 48.].into_iter().enumerate() {
                let at = square(12., 12. + i as f32 * 10., w, 4.);
                drawn = drawn.rect(at, theme.border);
            }
            drawn.rect(square(12., 74., 6., 10.), theme.accent)
        }
        4 => {
            let ground = height * 0.8;
            drawn = drawn.rect(
                square(0., ground, width, height - ground),
                theme.success_soft,
            );
            drawn.circle(point(px(width - 56.), px(44.)), px(20.), theme.warning)
        }
        _ => {
            for (i, (sx, sy)) in [
                (0.1, 0.2),
                (0.25, 0.5),
                (0.4, 0.15),
                (0.7, 0.3),
                (0.85, 0.6),
                (0.6, 0.7),
                (0.93, 0.12),
            ]
            .into_iter()
            .enumerate()
            {
                let r = if i % 3 == 0 { 2. } else { 1.5 };
                drawn = drawn.circle(point(px(width * sx), px(height * sy)), px(r), theme.faint);
            }
            drawn.circle(point(px(width - 50.), px(40.)), px(18.), theme.warning_soft)
        }
    }
}

/// Claude: a round creature in the agent's colour, with `face`.
pub(crate) fn creature(
    mut drawn: Canvas,
    (x, y): (f32, f32),
    r: f32,
    face: Face,
    theme: &Theme,
) -> Canvas {
    let at = |dx: f32, dy: f32| point(px(x + dx * r), px(y + dy * r));
    let ink = theme.foreground;
    drawn = drawn.circle(at(0., 0.), px(r), theme.agent);
    drawn = drawn.circle(at(0., 0.3), px(r * 0.55), theme.agent_soft);
    // eyes
    match face {
        Face::Happy | Face::Blush => {
            for side in [-0.35, 0.35] {
                drawn = drawn
                    .line(at(side - 0.18, -0.1), at(side, -0.25), px(3.), ink)
                    .line(at(side, -0.25), at(side + 0.18, -0.1), px(3.), ink);
            }
        }
        Face::Error => {
            for side in [-0.35, 0.35] {
                drawn = drawn
                    .line(at(side - 0.14, -0.32), at(side + 0.14, -0.04), px(3.), ink)
                    .line(at(side + 0.14, -0.32), at(side - 0.14, -0.04), px(3.), ink);
            }
        }
        Face::Shock => drawn = eyes(drawn, (x, y), r * 1.3, (0., 0.), theme),
        Face::Think => drawn = eyes(drawn, (x, y), r, (0.06, -0.06), theme),
        Face::Sad => drawn = eyes(drawn, (x, y), r, (0., 0.06), theme),
        Face::Plain => drawn = eyes(drawn, (x, y), r, (0., 0.), theme),
    }
    // mouth
    drawn = match face {
        Face::Plain => drawn.line(at(-0.15, 0.4), at(0.15, 0.4), px(3.), ink),
        Face::Happy | Face::Blush => drawn
            .line(at(-0.3, 0.3), at(-0.15, 0.45), px(3.), ink)
            .line(at(-0.15, 0.45), at(0.15, 0.45), px(3.), ink)
            .line(at(0.15, 0.45), at(0.3, 0.3), px(3.), ink),
        Face::Sad => drawn
            .line(at(-0.3, 0.5), at(-0.15, 0.38), px(3.), ink)
            .line(at(-0.15, 0.38), at(0.15, 0.38), px(3.), ink)
            .line(at(0.15, 0.38), at(0.3, 0.5), px(3.), ink),
        Face::Think => drawn.line(at(0.05, 0.42), at(0.3, 0.42), px(3.), ink),
        Face::Error => drawn
            .line(at(-0.3, 0.4), at(-0.15, 0.48), px(3.), ink)
            .line(at(-0.15, 0.48), at(0., 0.4), px(3.), ink)
            .line(at(0., 0.4), at(0.15, 0.48), px(3.), ink)
            .line(at(0.15, 0.48), at(0.3, 0.4), px(3.), ink),
        Face::Shock => drawn.circle(at(0., 0.45), px(r * 0.12), ink),
    };
    // what the face adds around the head
    match face {
        Face::Blush => {
            for side in [-0.6, 0.6] {
                drawn = drawn.circle(at(side, 0.15), px(r * 0.14), theme.danger.opacity(0.4));
            }
            drawn = heart(drawn, at(1.1, -0.9), r * 0.16, theme.danger);
        }
        Face::Think => {
            for (i, (dx, dy)) in [(0.9, -0.8), (1.1, -1.05), (1.35, -1.35)]
                .into_iter()
                .enumerate()
            {
                drawn = drawn.circle(at(dx, dy), px(r * (0.05 + 0.03 * i as f32)), theme.muted);
            }
        }
        Face::Error => {
            // overloaded: a bar over the head, mostly full
            drawn = drawn
                .rect(
                    square(x - r * 0.6, y - r * 1.5, r * 1.2, r * 0.18),
                    theme.danger_soft,
                )
                .rect(
                    square(x - r * 0.6, y - r * 1.5, r * 1.1, r * 0.18),
                    theme.danger,
                );
        }
        Face::Sad => drawn = drawn.circle(at(-0.42, 0.12), px(r * 0.06), theme.link),
        _ => {}
    }
    drawn
}

/// Round eyes on a head of `r` at `(x, y)`, the pupils looking `(dx, dy)`.
fn eyes(
    mut drawn: Canvas,
    (x, y): (f32, f32),
    r: f32,
    (dx, dy): (f32, f32),
    theme: &Theme,
) -> Canvas {
    for side in [-0.35, 0.35] {
        let eye = point(px(x + side * r), px(y - 0.15 * r));
        drawn = drawn.circle(eye, px(r * 0.18), theme.background).circle(
            point(px(x + (side + dx) * r), px(y + (dy - 0.15) * r)),
            px(r * 0.09),
            theme.foreground,
        );
    }
    drawn
}

/// A heart of `r`, its point at the bottom, from two discs and a block.
fn heart(drawn: Canvas, at: Point<Pixels>, r: f32, color: Hsla) -> Canvas {
    let (x, y) = (f32::from(at.x), f32::from(at.y));
    drawn
        .circle(point(px(x - r * 0.5), px(y - r * 0.3)), px(r * 0.55), color)
        .circle(point(px(x + r * 0.5), px(y - r * 0.3)), px(r * 0.55), color)
        .rect(square(x - r * 0.75, y - r * 0.3, r * 1.5, r * 0.6), color)
        .circle(point(px(x), px(y + r * 0.35)), px(r * 0.45), color)
}

/// The hub's picture of the game: Claude blushing, in the middle of its
/// 208 x 120 px.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    let drawn = canvas().size_full();
    creature(drawn, (104., 64.), 34., Face::Blush, theme)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;

    /// Every node a node can lead to, every branch of a gate and roll.
    fn targets(node: &Node) -> Vec<&'static str> {
        match node.next {
            Next::To(to) => vec![to],
            Next::Pick(choices) => choices.iter().map(|choice| choice.to).collect(),
            Next::Roll(ids) => ids.to_vec(),
            Next::Warm(_, yes, no) | Next::Burned(yes, no) => vec![yes, no],
            Next::End(_) => Vec::new(),
        }
    }

    #[test]
    fn the_script_holds_together() {
        let ids: Vec<&str> = NODES.iter().map(|node| node.id).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "an id twice");
        assert!(find(START).is_some());
        for node in NODES {
            assert!(!node.text.is_empty(), "{}: no line", node.id);
            assert!(
                (1..=CHAPTERS).contains(&(node.id.as_bytes()[0].wrapping_sub(b'0'))),
                "{}: no chapter digit",
                node.id
            );
            for to in targets(node) {
                assert!(find(to).is_some(), "{}: no node {to}", node.id);
            }
            if let Next::Pick(choices) = node.next {
                assert!(
                    (2..=3).contains(&choices.len()),
                    "{}: 2 or 3 choices",
                    node.id
                );
            }
        }
        // every node is reached from the start
        let mut seen = HashSet::from([START]);
        let mut stack = vec![START];
        while let Some(id) = stack.pop() {
            for to in targets(&NODES[find(id).unwrap()]) {
                if seen.insert(to) {
                    stack.push(to);
                }
            }
        }
        let unreached: Vec<&str> = ids
            .iter()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        assert!(unreached.is_empty(), "unreached: {unreached:?}");
        // every path ends: no cycle, and only an ending leads nowhere
        let mut done = HashSet::new();
        let mut path = Vec::new();
        fn walk(id: &'static str, path: &mut Vec<&'static str>, done: &mut HashSet<&'static str>) {
            if done.contains(id) {
                return;
            }
            assert!(!path.contains(&id), "a cycle: {path:?} -> {id}");
            path.push(id);
            let node = &NODES[find(id).unwrap()];
            let next = targets(node);
            assert!(
                !next.is_empty() || matches!(node.next, Next::End(_)),
                "{id} leads nowhere"
            );
            for to in next {
                walk(to, path, done);
            }
            path.pop();
            done.insert(id);
        }
        walk(START, &mut path, &mut done);
        // every ending is written
        let endings: BTreeSet<&str> = NODES
            .iter()
            .filter_map(|node| match node.next {
                Next::End(ending) => Some(ending.title()),
                _ => None,
            })
            .collect();
        let all: BTreeSet<&str> = [
            Ending::True,
            Ending::Quota,
            Ending::Refused,
            Ending::Table,
            Ending::Summary,
        ]
        .into_iter()
        .map(Ending::title)
        .collect();
        assert_eq!(endings, all);
    }

    /// The game played through: every line shown and gone past, and at
    /// each choice the next of `picks` taken. Stops at the ending's line.
    fn play(dating: &mut Dating, picks: &[usize]) {
        let mut picks = picks.iter();
        loop {
            while dating.tick() {}
            if matches!(dating.node().next, Next::End(_)) {
                return;
            }
            let key = if dating.choices().is_empty() {
                "space"
            } else {
                match picks.next() {
                    Some(0) => "1",
                    Some(1) => "2",
                    Some(_) => "3",
                    None => return,
                }
            };
            assert!(dating.key_down(key, false), "{key} at {}", dating.node().id);
        }
    }

    #[test]
    fn the_warmest_route_reaches_the_true_ending_and_scores_the_most() {
        let mut dating = Dating::new(7);
        // 사귀자, 항상 허용; 둘 다 써, 같이 밤새자; 당연히 너; ㄱㄱ, 응 좋았지
        play(&mut dating, &[2, 2, 2, 1, 0, 2, 1]);
        assert_eq!(dating.node().id, "5-confess");
        assert_eq!(dating.affection, 46);
        assert!(!dating.burned);
        // 좋아해: the table is begun and dropped
        play(&mut dating, &[0]);
        assert_eq!(dating.node().id, "5-true2");
        assert!(!dating.over());
        assert_eq!(dating.score(), 0, "not before the ending is read");
        assert!(dating.key_down("space", false));
        assert_eq!(dating.ending(), Some(Ending::True));
        assert_eq!(dating.score(), 1_460);
        assert_eq!(dating.period(), None);
        assert!(!dating.key_down("space", false), "over: nothing more");
    }

    #[test]
    fn merge_is_approved_at_thirty_and_reviewed_under_it() {
        // 사귀자, y; 둘 다 써, 같이 밤새자; 당연히 너; 한강, 환각이야: 35
        let mut warm = Dating::new(7);
        play(&mut warm, &[2, 0, 2, 1, 0, 0, 0, 1]);
        assert_eq!(warm.affection, 35);
        assert_eq!(warm.node().id, "5-true2");
        // the same but for 안녕 and 탭: 25
        let mut cool = Dating::new(7);
        play(&mut cool, &[0, 0, 1, 0, 0, 0, 1]);
        assert_eq!(cool.affection, 25);
        assert_eq!(cool.node().id, "5-merge-review2");
    }

    #[test]
    fn a_burned_quota_ends_at_the_confession_whatever_the_affection() {
        let mut dating = Dating::new(7);
        // the warmest route but for 자율주행
        play(&mut dating, &[2, 2, 2, 0, 0, 2, 1]);
        assert!(dating.burned);
        assert_eq!(dating.affection, 45);
        assert_eq!(dating.node().id, "5-quota4");
        assert!(dating.key_down("space", false));
        assert_eq!(dating.ending(), Some(Ending::Quota));
        assert_eq!(dating.score(), 650);
    }

    #[test]
    fn a_cold_route_is_refused_and_a_warm_one_answered_with_a_table() {
        let mut dating = Dating::new(7);
        // 안녕, 탭, /clear, Haiku, 그냥 네가 정해, 너 지금 환각 났어: -2, then 좋아해
        play(&mut dating, &[0, 0, 2, 2, 1, 2, 0]);
        assert_eq!(dating.affection, -2);
        assert_eq!(dating.node().id, "5-refuse3");
        assert!(dating.key_down("space", false));
        assert_eq!(dating.ending(), Some(Ending::Refused));
        assert_eq!(dating.score(), 100);
        // 안녕, 탭, 같이 밤새자, 당연히 너, 한강, 응 좋았지: 28, then 좋아해
        let mut dating = Dating::new(7);
        play(&mut dating, &[0, 0, 1, 0, 0, 1, 0]);
        assert_eq!(dating.affection, 28);
        assert_eq!(dating.node().id, "5-table3");
        assert!(dating.key_down("space", false));
        assert_eq!(dating.ending(), Some(Ending::Table));
        assert_eq!(dating.score(), 580);
    }

    #[test]
    fn a_line_reveals_a_character_a_tick_on_the_clock_and_space_finishes_it() {
        let mut dating = Dating::new(1);
        let len = dating.node().text.chars().count();
        assert_eq!(dating.period(), Some(PERIOD));
        assert!(dating.tick());
        assert_eq!(dating.shown, 1);
        assert!(dating.key_down("space", false));
        assert_eq!(dating.shown, len);
        assert_eq!(dating.period(), None, "shown: no clock");
        assert!(!dating.tick());
        // the next press goes on, and the clock is asked for again
        assert!(dating.key_down("space", false));
        assert_eq!(dating.node().id, "1-hi");
        assert_eq!(dating.shown, 0);
        assert_eq!(dating.period(), Some(PERIOD));
        assert!(!dating.key_down("space", true), "a repeat does nothing");
    }

    #[test]
    fn the_arrows_pick_among_the_choices_once_the_line_is_shown() {
        let mut dating = Dating::new(1);
        while dating.tick() {}
        dating.key_down("space", false);
        assert_eq!(dating.node().id, "1-hi");
        assert!(dating.choices().is_empty(), "not before the line is shown");
        assert!(!dating.key_down("down", false));
        while dating.tick() {}
        assert_eq!(dating.choices().len(), 3);
        assert!(dating.key_down("up", false));
        assert_eq!(dating.cursor, 2);
        assert!(dating.key_down("down", false));
        assert_eq!(dating.cursor, 0);
        assert!(dating.key_down("down", false));
        assert!(dating.key_down("enter", false));
        assert_eq!(dating.node().id, "1-bug");
        assert_eq!(dating.affection, 2);
    }

    #[test]
    fn the_seed_rolls_the_lines_that_vary() {
        let rolled = |seed: u64| {
            let mut dating = Dating::new(seed);
            // 안녕 is answered one of two ways
            for key in ["space", "1", "space"] {
                while dating.tick() {}
                dating.key_down(key, false);
            }
            dating.node().id
        };
        let seen: HashSet<&str> = (0..40).map(rolled).collect();
        assert!(seen.len() > 1, "{seen:?}");
        assert_eq!(rolled(3), rolled(3));
    }
}
