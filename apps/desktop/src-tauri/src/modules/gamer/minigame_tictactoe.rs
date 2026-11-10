//! Payload HTML do jogo da velha — porte literal de `handlers/tictactoe-game.ts`. Auto-contido, sem
//! I/O. Script em arquivo separado ([`super::minigame_tictactoe_script`]) — ver o cabeçalho de lá.

use super::minigame_shell::GAME_SHELL_BODY_CSS;
use super::minigame_tictactoe_script::TICTACTOE_SCRIPT;

const TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    __SHELL_CSS__
    .container {
      background: #09090b;
      border: 3px solid #ec4899;
      box-shadow: 0 0 20px rgba(236, 72, 153, 0.4);
      padding: 30px;
      border-radius: 12px;
      text-align: center;
    }
    .board {
      display: grid;
      grid-template-columns: repeat(3, 100px);
      grid-gap: 10px;
      margin: 20px 0;
    }
    .cell {
      width: 100px;
      height: 100px;
      background: #1f2937;
      border: 2px solid #f472b6;
      border-radius: 8px;
      font-size: 36px;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      font-weight: bold;
      transition: all 0.2s ease;
    }
    .cell:hover {
      background: #374151;
    }
    .cell.x {
      color: #3b82f6;
      text-shadow: 0 0 10px #3b82f6;
    }
    .cell.o {
      color: #ec4899;
      text-shadow: 0 0 10px #ec4899;
    }
    .status {
      font-size: 20px;
      height: 30px;
      font-weight: bold;
      color: #f472b6;
    }
    .btn {
      background: #ec4899;
      border: none;
      color: white;
      padding: 10px 20px;
      cursor: pointer;
      border-radius: 6px;
      font-size: 16px;
      margin-top: 10px;
      font-weight: bold;
      transition: opacity 0.2s;
    }
    .btn:hover {
      opacity: 0.9;
    }
  </style>
</head>
<body>
  <div class="container">
    <h2>Jogo da Velha</h2>
    <div class="status" id="status">Sua vez! (X)</div>
    <div class="board" id="board">
      <div class="cell" onclick="makeMove(0)"></div>
      <div class="cell" onclick="makeMove(1)"></div>
      <div class="cell" onclick="makeMove(2)"></div>
      <div class="cell" onclick="makeMove(3)"></div>
      <div class="cell" onclick="makeMove(4)"></div>
      <div class="cell" onclick="makeMove(5)"></div>
      <div class="cell" onclick="makeMove(6)"></div>
      <div class="cell" onclick="makeMove(7)"></div>
      <div class="cell" onclick="makeMove(8)"></div>
    </div>
    <button class="btn" onclick="resetBoard()">Reiniciar</button>
  </div>

  <script>__TICTACTOE_SCRIPT__  </script>
</body>
</html>"#;

pub fn tictactoe_html() -> String {
    TEMPLATE.replace("__SHELL_CSS__", GAME_SHELL_BODY_CSS).replace("__TICTACTOE_SCRIPT__", TICTACTOE_SCRIPT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_shared_shell_css_and_the_game_script() {
        let html = tictactoe_html();
        assert!(html.contains("font-family: 'Courier New', monospace"));
        assert!(html.contains("function makeMove(index)"));
    }

    #[test]
    fn has_nine_clickable_cells_and_a_reset_button() {
        let html = tictactoe_html();
        assert_eq!(html.matches("onclick=\"makeMove(").count(), 9);
        assert!(html.contains("onclick=\"resetBoard()\""));
    }
}
