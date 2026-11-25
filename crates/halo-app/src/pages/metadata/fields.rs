//! Metadata editor — tag fields: Title, Artist, Album artist, Album,
//! Genre | Composer, Year + Track #/of + Disc #/of, Comment, Publisher |
//! Copyright, Language (with the language-code suggestions list).

use iced::widget::{button, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Alignment, Element, Length, Padding};

use super::{mm, Draft, Field, Msg, TITLE_INPUT};
use crate::app::{Halo, Message};
use crate::theme::{self, t};
use crate::ui;

pub const LANGUAGES: [(&str, &str); 27] = [
    ("tam", "Tamil"),
    ("eng", "English"),
    ("tel", "Telugu"),
    ("mal", "Malayalam"),
    ("hin", "Hindi"),
    ("kan", "Kannada"),
    ("ben", "Bengali"),
    ("mar", "Marathi"),
    ("guj", "Gujarati"),
    ("pan", "Punjabi"),
    ("urd", "Urdu"),
    ("ara", "Arabic"),
    ("fra", "French"),
    ("deu", "German"),
    ("spa", "Spanish"),
    ("por", "Portuguese"),
    ("ita", "Italian"),
    ("rus", "Russian"),
    ("jpn", "Japanese"),
    ("kor", "Korean"),
    ("zho", "Chinese"),
    ("tha", "Thai"),
    ("vie", "Vietnamese"),
    ("ind", "Indonesian"),
    ("pol", "Polish"),
    ("nld", "Dutch"),
    ("swe", "Swedish"),
];

fn input<'a>(placeholder: &'a str, value: &'a str, f: Field) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| mm(Msg::Field(f, v)))
        .on_submit(mm(Msg::Save))
        .size(theme::BODY)
        .padding(Padding::from([4.0, 8.0]))
        .style(theme::input_muted)
}

fn numeric<'a>(placeholder: &'a str, value: &'a str, f: Field) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| mm(Msg::Field(f, v.chars().filter(|c| c.is_ascii_digit()).collect())))
        .on_submit(mm(Msg::Save))
        .size(theme::BODY)
        .padding(Padding::from([4.0, 8.0]))
        .style(theme::input_muted)
}

/// Labelled field (`text-xs text-muted-foreground font-medium` label).
fn f<'a>(label: &'a str, el: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        text(label).size(theme::SMALL).font(theme::MEDIUM).color(t().muted_fg),
        el.into(),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn pair<'a>(a: Element<'a, Message>, b: Element<'a, Message>) -> Element<'a, Message> {
    row![a, b].spacing(10).into()
}

fn slash<'a>(a: text_input::TextInput<'a, Message>, b: text_input::TextInput<'a, Message>) -> Element<'a, Message> {
    row![
        a.width(Length::Fill),
        text("/").size(theme::SMALL).color(t().muted_fg),
        b.width(Length::Fill),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

pub fn view<'a>(app: &'a Halo, d: &'a Draft) -> Element<'a, Message> {
    let k = t();
    // Language suggestions matching what's typed (datalist equivalent).
    let q = d.language.trim().to_lowercase();
    let suggestions: Vec<(&str, &str)> = if q.is_empty() || LANGUAGES.iter().any(|(c, _)| *c == q) {
        vec![]
    } else {
        LANGUAGES
            .iter()
            .filter(|(c, l)| c.starts_with(&q) || l.to_lowercase().starts_with(&q))
            .copied()
            .take(6)
            .collect()
    };
    let mut lang = column![input("e.g. eng", &d.language, Field::Language)].spacing(4);
    if !suggestions.is_empty() {
        let mut chips = row![].spacing(4);
        for (code, label) in suggestions {
            chips = chips.push(
                button(text(format!("{code} · {label}")).size(theme::SMALL))
                    .padding(Padding::from([2.0, 8.0]))
                    .style(theme::btn_chip(false, theme::R_FULL))
                    .on_press(mm(Msg::Field(Field::Language, code.to_string()))),
            );
        }
        lang = lang.push(chips.wrap().vertical_spacing(4));
    }

    let form = column![
        f("Title", input("Track title", &d.title, Field::Title).id(TITLE_INPUT)),
        f("Artist", input("Artist, comma-separated", &d.artists, Field::Artists)),
        f("Album artist", input("Album artist, comma-separated", &d.album_artists, Field::AlbumArtists)),
        f("Album", input("Album", &d.album_name, Field::Album)),
        pair(
            f("Genre", input("Genre, comma-separated", &d.genres, Field::Genres)),
            f("Composer", input("Composer, comma-separated", &d.composers, Field::Composers)),
        ),
        row![
            container(f("Year", numeric("Year", &d.year, Field::Year))).width(72),
            f("Track #", slash(
                numeric("#", &d.track_number, Field::TrackNumber),
                numeric("of", &d.track_total, Field::TrackTotal),
            )),
            f("Disc #", slash(
                numeric("#", &d.disc_number, Field::DiscNumber),
                numeric("of", &d.disc_total, Field::DiscTotal),
            )),
        ]
        .spacing(8)
        .align_y(Alignment::End),
        f(
            "Comment",
            text_editor(&d.comment)
                .on_action(|a| mm(Msg::Comment(a)))
                .placeholder("Comment")
                .size(theme::BODY)
                .padding(Padding::from([4.0, 8.0]))
                .height(52)
                .style(theme::editor),
        ),
        pair(
            f("Publisher / Label", input("Record label", &d.publisher, Field::Publisher)),
            f("Copyright", input("© 2025 …", &d.copyright, Field::Copyright)),
        ),
        f("Language", lang),
    ]
    .spacing(10)
    .padding(Padding::ZERO.right(12.0));

    let max_h = (app.main_size.height * 0.62).max(200.0);
    let _ = (k, ui::fill());
    container(
        scrollable(form)
            .direction(theme::vscroll())
            .style(theme::scroll)
            .height(Length::Shrink),
    )
    .max_height(max_h)
    .width(Length::Fill)
    .into()
}
