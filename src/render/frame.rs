use tiny_skia::Pixmap;
use crate::config::*;
use crate::data::manifest::Segment;
use crate::data::text_chunker::TextChunk;
use crate::render::background::render_background;
use crate::render::cards::{render_intro_card, render_section_card, render_outro_card, render_paragraph};
use crate::render::text::TextRenderer;

/// Fade curve for text-chunk swaps inside a single paragraph.
/// Returns 1.0 everywhere except within CHUNK_CROSSFADE_FRAMES/2 of a chunk
/// boundary, where it dips to 0 at the boundary itself. This makes the
/// instantaneous text swap invisible — by the time a reader sees the new
/// chunk, the old one has faded out and faded back in.
fn chunk_fade_multiplier(local_frame: u32, text_chunks: &[TextChunk]) -> f32 {
    if text_chunks.len() < 2 {
        return 1.0;
    }
    let half = (CHUNK_CROSSFADE_FRAMES / 2).max(1);
    // Skip the first chunk — index 0 has no incoming boundary to fade over.
    for chunk in text_chunks.iter().skip(1) {
        let boundary = chunk.start_frame as i32;
        let delta = (local_frame as i32 - boundary).unsigned_abs();
        if delta < half {
            return delta as f32 / half as f32;
        }
    }
    1.0
}

/// Calculate fade opacity for a frame within a segment.
/// Returns 0.0-1.0 based on position within the segment.
pub fn fade_opacity(local_frame: u32, duration_frames: u32) -> f32 {
    let fade = FADE_FRAMES;

    if duration_frames <= fade * 2 {
        // Very short segment — just do a simple triangle
        let mid = duration_frames / 2;
        if local_frame < mid {
            local_frame as f32 / mid as f32
        } else {
            (duration_frames - local_frame) as f32 / (duration_frames - mid) as f32
        }
    } else if local_frame < fade {
        // Fade in
        local_frame as f32 / fade as f32
    } else if local_frame > duration_frames - fade {
        // Fade out
        (duration_frames - local_frame) as f32 / fade as f32
    } else {
        1.0
    }
}

/// Render a single complete frame for a given segment and local frame offset.
pub fn render_frame(
    renderer: &mut TextRenderer,
    segment: &Segment,
    local_frame: u32,
    global_time_sec: f64,
) -> Pixmap {
    // Background
    let mut pixmap = render_background(global_time_sec);

    let opacity = match segment {
        // Intro: no fade-in (visible from frame 0 for YouTube thumbnails), only fade-out
        Segment::Intro { duration_frames, .. } => {
            let fade = FADE_FRAMES;
            if local_frame >= duration_frames - fade {
                (duration_frames - local_frame) as f32 / fade as f32
            } else {
                1.0
            }
        }
        Segment::SectionCard { duration_frames, .. } => fade_opacity(local_frame, *duration_frames),
        Segment::Paragraph { duration_frames, text_chunks, .. } => {
            fade_opacity(local_frame, *duration_frames)
                * chunk_fade_multiplier(local_frame, text_chunks)
        }
        Segment::Outro { duration_frames, .. } => fade_opacity(local_frame, *duration_frames),
    };

    // Render content onto a separate layer
    let mut content = Pixmap::new(WIDTH, HEIGHT).unwrap();

    match segment {
        Segment::Intro {
            paper_title,
            paper_id,
            ..
        } => {
            render_intro_card(renderer, &mut content, paper_id, paper_title);
        }
        Segment::SectionCard { section_title, .. } => {
            render_section_card(renderer, &mut content, section_title);
        }
        Segment::Paragraph {
            text,
            standard_reference_id,
            text_chunks,
            duration_frames,
            ..
        } => {
            // Find the active text chunk
            let active_chunk = text_chunks
                .iter()
                .rev()
                .find(|c| local_frame >= c.start_frame)
                .unwrap_or(&text_chunks[0]);

            render_paragraph(
                renderer,
                &mut content,
                &active_chunk.text,
                standard_reference_id,
            );
        }
        Segment::Outro { tagline, .. } => {
            render_outro_card(renderer, &mut content, tagline.as_deref());
        }
    }

    // Composite content onto background with opacity
    crate::render::compositor::composite(&mut pixmap, &content, opacity);

    pixmap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::text_chunker::TextChunk;

    #[test]
    fn test_render_intro_frame() {
        let mut renderer = TextRenderer::new();
        let segment = Segment::Intro {
            paper_title: "The Universal Father".to_string(),
            paper_id: "1".to_string(),
            start_frame: 0,
            duration_frames: 150,
        };
        let pixmap = render_frame(&mut renderer, &segment, 75, 2.5);
        pixmap.save_png("output/test_intro_frame.png").unwrap();
    }

    #[test]
    fn test_render_paragraph_frame() {
        let mut renderer = TextRenderer::new();
        let text = "THE Universal Father is the God of all creation, the First Source and Center of all things and beings. First think of God as a creator, then as a controller, and lastly as an infinite upholder.";
        let segment = Segment::Paragraph {
            global_id: "1:1.0.1".to_string(),
            standard_reference_id: "1:0.1".to_string(),
            text: text.to_string(),
            section_title: None,
            audio_duration_sec: 15.0,
            start_frame: 150,
            duration_frames: 450,
            text_chunks: vec![TextChunk {
                text: text.to_string(),
                start_frame: 0,
                duration_frames: 450,
            }],
        };
        let pixmap = render_frame(&mut renderer, &segment, 225, 10.0);
        pixmap.save_png("output/test_paragraph_frame.png").unwrap();
    }

    #[test]
    fn test_render_outro_frame() {
        let mut renderer = TextRenderer::new();
        let segment = Segment::Outro {
            start_frame: 0,
            duration_frames: 150,
            tagline: None,
        };
        let pixmap = render_frame(&mut renderer, &segment, 75, 60.0);
        pixmap.save_png("output/test_outro_frame.png").unwrap();
    }

    #[test]
    fn test_render_long_paragraph() {
        // 176:3.4 — longest paragraph in the book (530 tokens)
        let text = "\u{201c}As individuals, and as a generation of believers, hear me while I speak a parable: There was a certain great man who, before starting out on a long journey to another country, called all his trusted servants before him and delivered into their hands all his goods. To one he gave five talents, to another two, and to another one. And so on down through the entire group of honored stewards, to each he intrusted his goods according to their several abilities; and then he set out on his journey. When their lord had departed, his servants set themselves at work to gain profits from the wealth intrusted to them. Immediately he who had received five talents began to trade with them and very soon had made a profit of another five talents. In like manner he who had received two talents soon had gained two more. And so did all of these servants make gains for their master except him who received but one talent. He went away by himself and dug a hole in the earth where he hid his lord\u{2019}s money. Presently the lord of those servants unexpectedly returned and called upon his stewards for a reckoning. And when they had all been called before their master, he who had received the five talents came forward with the money which had been intrusted to him and brought five additional talents, saying, \u{2018}Lord, you gave me five talents to invest, and I am glad to present five other talents as my gain.\u{2019} And then his lord said to him: \u{2018}Well done, good and faithful servant, you have been faithful over a few things; I will now set you as steward over many; enter forthwith into the joy of your lord.\u{2019} And then he who had received the two talents came forward, saying: \u{2018}Lord, you delivered into my hands two talents; behold, I have gained these other two talents.\u{2019} And his lord then said to him: \u{2018}Well done, good and faithful steward; you also have been faithful over a few things, and I will now set you over many; enter you into the joy of your lord.\u{2019} And then there came to the accounting he who had received the one talent. This servant came forward, saying, \u{2018}Lord, I knew you and realized that you were a shrewd man in that you expected gains where you had not personally labored; therefore was I afraid to risk aught of that which was intrusted to me. I safely hid your talent in the earth; here it is; you now have what belongs to you.\u{2019} But his lord answered: \u{2018}You are an indolent and slothful steward. By your own words you confess that you knew I would require of you an accounting with reasonable profit, such as your diligent fellow servants have this day rendered. Knowing this, you ought, therefore, to have at least put my money into the hands of the bankers that on my return I might have received my own with interest.\u{2019} And then to the chief steward this lord said: \u{2018}Take away this one talent from this unprofitable servant and give it to him who has the ten talents.\u{2019}\u{201d}";
        let mut renderer = TextRenderer::new();
        let segment = Segment::Paragraph {
            global_id: "3:176.3.4".to_string(),
            standard_reference_id: "176:3.4".to_string(),
            text: text.to_string(),
            section_title: None,
            audio_duration_sec: 45.0,
            start_frame: 0,
            duration_frames: 1350,
            text_chunks: vec![TextChunk {
                text: text.to_string(),
                start_frame: 0,
                duration_frames: 1350,
            }],
        };
        let pixmap = render_frame(&mut renderer, &segment, 675, 30.0);
        pixmap.save_png("output/test_long_paragraph.png").unwrap();
    }

}
