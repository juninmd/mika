//! Payload HTML do jokenpo — porte literal de `handlers/jokenpo-game.ts`. Auto-contido (HTML + CSS +
//! JS embutidos), sem I/O: o jogo roda inteiro no `<iframe>`/webview que recebe este HTML.

use super::minigame_shell::GAME_SHELL_BODY_CSS;

const TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    __SHELL_CSS__
    .container {
      background: #09090b;
      border: 3px solid #8b5cf6;
      box-shadow: 0 0 20px rgba(139, 92, 246, 0.4);
      padding: 30px;
      border-radius: 12px;
      text-align: center;
      width: 400px;
    }
    .options {
      display: flex;
      justify-content: space-around;
      margin: 25px 0;
    }
    button {
      background: #1f2937;
      border: 2px solid #a78bfa;
      color: white;
      padding: 15px 20px;
      font-size: 24px;
      cursor: pointer;
      border-radius: 8px;
      transition: all 0.2s ease;
    }
    button:hover {
      background: #8b5cf6;
      box-shadow: 0 0 10px #8b5cf6;
      transform: scale(1.1);
    }
    .result {
      font-size: 20px;
      font-weight: bold;
      margin-top: 20px;
      min-height: 40px;
      color: #10b981;
    }
    .choices {
      margin-top: 15px;
      font-size: 16px;
      color: #9ca3af;
    }
    .score {
      font-size: 18px;
      margin-bottom: 10px;
    }
  </style>
</head>
<body>
  <div class="container">
    <h2>Pedra, Papel ou Tesoura</h2>
    <div class="score">Placar - Você: <span id="p-score">0</span> | Mika: <span id="m-score">0</span></div>
    <div class="options">
      <button onclick="play('🪨')">🪨</button>
      <button onclick="play('📄')">📄</button>
      <button onclick="play('✂️')">✂️</button>
    </div>
    <div class="choices" id="choices-show">Escolha uma opção para começar!</div>
    <div class="result" id="game-result"></div>
  </div>

  <script>
    let pScore = 0;
    let mScore = 0;
    const items = ['🪨', '📄', '✂️'];
    const mappings = { '🪨': 'Pedra', '📄': 'Papel', '✂️': 'Tesoura' };

    function play(playerChoice) {
      const mikaChoice = items[Math.floor(Math.random() * 3)];
      document.getElementById('choices-show').innerText = 'Você escolheu: ' + mappings[playerChoice] + ' | Mika escolheu: ' + mappings[mikaChoice];

      let res = '';
      let outcome = 'draw';
      if (playerChoice === mikaChoice) {
        res = 'Empate! 🤝';
        document.getElementById('game-result').style.color = '#eab308';
      } else if (
        (playerChoice === '🪨' && mikaChoice === '✂️') ||
        (playerChoice === '📄' && mikaChoice === '🪨') ||
        (playerChoice === '✂️' && mikaChoice === '📄')
      ) {
        res = 'Você venceu! 🎉';
        outcome = 'win';
        pScore++;
        document.getElementById('p-score').innerText = pScore;
        document.getElementById('game-result').style.color = '#10b981';
      } else {
        res = 'Mika venceu! 😈';
        outcome = 'lose';
        mScore++;
        document.getElementById('m-score').innerText = mScore;
        document.getElementById('game-result').style.color = '#ef4444';
      }
      document.getElementById('game-result').innerText = res;
      window.parent.postMessage({ game: 'jokenpo', result: outcome, pScore: pScore, mScore: mScore }, '*');
    }
  </script>
</body>
</html>"#;

pub fn jokenpo_html() -> String {
    TEMPLATE.replace("__SHELL_CSS__", GAME_SHELL_BODY_CSS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_shared_shell_css() {
        assert!(jokenpo_html().contains("font-family: 'Courier New', monospace"));
    }

    #[test]
    fn keeps_the_three_emoji_choice_buttons_and_the_result_message_channel() {
        let html = jokenpo_html();
        assert!(html.contains("play('🪨')"));
        assert!(html.contains("play('📄')"));
        assert!(html.contains("play('✂️')"));
        assert!(html.contains("window.parent.postMessage({ game: 'jokenpo'"));
    }
}
