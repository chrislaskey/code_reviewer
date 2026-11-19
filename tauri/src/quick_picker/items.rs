//! The items the picker lists: the posts the user sees, as text, and
//! the user's files that are images the clipboard takes.

use chrono::{DateTime, Utc};

use app_core::deployment::Deployment;
use app_core::files::{self, File};
use app_core::posts::{self, Post};
use app_core::users::User;

use crate::clipboard;

use super::{Item, ItemKind};

pub async fn list(deployment: &Deployment, user: &User) -> Result<Vec<Item>, String> {
    let posts = posts::list(deployment, user)
        .await
        .map_err(|error| error.to_string())?;
    let files = files::list(deployment, user)
        .await
        .map_err(|error| error.to_string())?;
    Ok(newest_first(
        posts.into_iter().map(|shown| shown.post).collect(),
        files.into_iter().map(|shown| shown.file).collect(),
    ))
}

/// The posts and the images together, the last changed first.
fn newest_first(posts: Vec<Post>, files: Vec<File>) -> Vec<Item> {
    let texts = posts.into_iter().map(|post| {
        let item = Item {
            kind: ItemKind::Text,
            id: post.id,
            label: post.body,
            size: None,
        };
        (post.updated_at, item)
    });
    let images = files
        .into_iter()
        .filter(|file| clipboard::uniform_type_of(&file.content_type).is_some())
        .map(|file| {
            let item = Item {
                kind: ItemKind::Image,
                id: file.id,
                label: file.name,
                size: Some(file.size),
            };
            (file.updated_at, item)
        });
    let mut dated: Vec<(DateTime<Utc>, Item)> = texts.chain(images).collect();
    dated.sort_by_key(|(updated_at, _item)| std::cmp::Reverse(*updated_at));
    dated.into_iter().map(|(_updated_at, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use uuid::Uuid;

    use super::*;

    fn day(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, 0, 0, 0)
            .single()
            .expect("a date")
    }

    fn post(body: &str, updated_at: DateTime<Utc>) -> Post {
        Post {
            id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(1),
            body: body.to_string(),
            inserted_at: updated_at,
            updated_at,
            version: 0,
            last_synced_at: None,
            deleted_at: None,
            deleted_by_id: None,
        }
    }

    fn file(name: &str, content_type: &str, updated_at: DateTime<Utc>) -> File {
        File {
            id: Uuid::from_u128(1),
            created_by_id: Uuid::from_u128(1),
            name: name.to_string(),
            content_type: content_type.to_string(),
            size: 2048,
            inserted_at: updated_at,
            updated_at,
            version: 0,
            last_synced_at: None,
            deleted_at: None,
            deleted_by_id: None,
        }
    }

    #[test]
    fn quick_picker_items_are_posts_and_images_the_last_changed_first() {
        let posts = vec![post("newest", day(3)), post("oldest", day(1))];
        let files = vec![file("cat.gif", "image/gif", day(2))];

        let items = newest_first(posts, files);

        let labels: Vec<_> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, ["newest", "cat.gif", "oldest"]);
        assert_eq!(items[0].kind, ItemKind::Text);
        assert_eq!(items[0].size, None);
        assert_eq!(items[1].kind, ItemKind::Image);
        assert_eq!(items[1].size, Some(2048));
    }

    #[test]
    fn quick_picker_items_leave_out_files_the_clipboard_does_not_take() {
        let files = vec![
            file("notes.txt", "text/plain", day(1)),
            file("drawing.svg", "image/svg+xml", day(2)),
            file("photo.jpg", "image/jpeg", day(3)),
        ];

        let items = newest_first(Vec::new(), files);

        let labels: Vec<_> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, ["photo.jpg"]);
    }
}
