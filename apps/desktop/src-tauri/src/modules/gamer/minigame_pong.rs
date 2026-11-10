//! Payload HTML do pong — porte literal de `handlers/pong-game.ts`. Auto-contido, sem I/O. Script em
//! arquivo separado ([`super::minigame_pong_script`]) — ver o cabeçalho de lá.

use super::minigame_pong_script::{PONG_SCRIPT, WIN_SCORE};
use super::minigame_shell::GAME_SHELL_BODY_CSS;

const TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    __SHELL_CSS__
    #game-container {
      position: relative;
      border: 4px solid #8b5cf6;
      box-shadow: 0 0 20px rgba(139, 92, 246, 0.4);
      background: #000;
    }
    canvas {
      display: block;
    }
    .score-board {
      display: flex;
      justify-content: space-between;
      width: 600px;
      font-size: 32px;
      font-weight: bold;
      margin-bottom: 10px;
      text-shadow: 0 0 10px #8b5cf6;
    }
    .instructions {
      margin-top: 15px;
      font-size: 14px;
      color: #a78bfa;
      text-align: center;
    }
    .status {
      margin-top: 10px;
      font-size: 20px;
      font-weight: bold;
      min-height: 28px;
    }
  </style>
</head>
<body>
  <div class="score-board">
    <span>Player: <span id="player-score">0</span></span>
    <span>Mika: <span id="mika-score">0</span></span>
  </div>
  <div id="game-container">
    <canvas id="pong" width="600" height="400"></canvas>
  </div>
  <div class="status" id="status"></div>
  <div class="instructions">Use UP/DOWN or W/S keys to move. First to __WIN_SCORE__ wins. Beat Mika!</div>

  <script>__PONG_SCRIPT__  </script>
</body>
</html>"#;

pub fn pong_html() -> String {
    TEMPLATE
        .replace("__SHELL_CSS__", GAME_SHELL_BODY_CSS)
        .replace("__PONG_SCRIPT__", PONG_SCRIPT)
        .replace("__WIN_SCORE__", &WIN_SCORE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_shared_shell_css_and_the_game_script() {
        let html = pong_html();
        assert!(html.contains("font-family: 'Courier New', monospace"));
        assert!(html.contains("function update()"));
    }

    #[test]
    fn interpolates_the_win_score_into_both_the_instructions_and_the_script() {
        let html = pong_html();
        assert!(html.contains("First to 7 wins"));
        assert!(html.contains("const WIN_SCORE = 7;"));
    }
}
