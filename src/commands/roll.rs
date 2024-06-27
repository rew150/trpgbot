use poise::{
    serenity_prelude::{Colour, CreateEmbed},
    CreateReply,
};

use super::{Context, Result};

fn embed_result(text: String) -> CreateReply {
    CreateReply::default()
        .embed(
            CreateEmbed::default()
                .color(Colour::from_rgb(170, 255, 0))
                .title("Roll")
                .description(text),
        )
        .reply(true)
}

#[poise::command(slash_command)]
pub async fn roll(
    ctx: Context<'_>,
    #[description = "Dice notation"] notation: String,
) -> Result<()> {
    let repo = ctx.data().nist_repo.clone();

    let mut ns = &notation[..];
    let mut raw_notations = vec![];
    let mut start_from_1 = false;

    while !ns.is_empty() {
        let fns = if start_from_1 { &ns[1..] } else { ns };
        match fns.find(&['+', '-']) {
            Some(i) => {
                let i = if start_from_1 { i + 1 } else { i };
                raw_notations.push(&ns[..i]);
                ns = &ns[i..];
                start_from_1 = true;
            }
            None => {
                raw_notations.push(ns);
                break;
            }
        }
    }

    let mut dice: Vec<Dice> = vec![];
    let mut modifiers: Vec<i64> = vec![];
    for n in raw_notations {
        let n: String = n.chars().filter(|c| !c.is_whitespace()).collect();
        if let Ok(i) = n.parse::<i64>() {
            modifiers.push(i);
        } else if let Some(d) = parse_dice(&n) {
            dice.push(d);
        } else {
            ctx.reply(&format!("Could not parse notation: {}", &notation))
                .await?;
            return Ok(());
        }
    }

    let mut total_value = 0;
    let mut results = String::new();

    for d in dice {
        let dstr = d.to_string();
        results.push_str("`[");
        results.push_str(&dstr);
        results.push_str("]`: ");

        let mut rolled = Vec::with_capacity(d.count);
        for _ in 0..d.count {
            let x = repo.rand(1, d.face).await?;
            if x == 1 || x == d.face {
                rolled.push(format!("**({x})**"));
            } else {
                rolled.push(x.to_string());
            }
            total_value += x;
        }

        results.push_str(&rolled.join(", "));
        results.push('\n');
    }

    let modded = modifiers
        .into_iter()
        .enumerate()
        .fold(String::new(), |acc, (i, x)| {
            total_value += x;

            if i == 0 {
                return x.to_string();
            }
            if x < 0 {
                format!("{} - {}", acc, -x)
            } else {
                format!("{} + {}", acc, x)
            }
        });

    if !modded.is_empty() {
        results.push_str("\nmodifier: ");
        results.push_str(&modded);
        results.push('\n');
    }

    results.push_str(&format!("\nResults: **{total_value}**"));

    ctx.send(embed_result(results)).await?;

    Ok(())
}

struct Dice {
    face: i64,
    count: usize,
}

impl std::fmt::Display for Dice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}d{}", self.count, self.face)
    }
}

fn parse_dice(d: &str) -> Option<Dice> {
    let d = d.trim().to_ascii_lowercase();
    let mut face = 0;
    let mut count = 0;
    for (i, s) in d.split('d').enumerate() {
        match i {
            0 => {
                count = s.parse().ok()?;
            }
            1 => {
                face = s.parse().ok()?;
            }
            _ => return None,
        }
    }
    if face <= 0 || count == 0 {
        return None;
    }

    Some(Dice { face, count })
}
