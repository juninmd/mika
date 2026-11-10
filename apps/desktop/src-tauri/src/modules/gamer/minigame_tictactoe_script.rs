//! Script do jogo da velha — porte literal de `handlers/tictactoe-script.ts`. Separado do template
//! HTML ([`super::minigame_tictactoe`]) só para manter os dois arquivos abaixo do teto de 300 linhas
//! do AGENTS.md.

pub const TICTACTOE_SCRIPT: &str = r#"
    let board = ['', '', '', '', '', '', '', '', ''];
    let gameActive = true;
    let generation = 0;
    let mikaMoveTimeoutId = null;
    const player = 'X';
    const mika = 'O';

    const winConditions = [
      [0, 1, 2], [3, 4, 5], [6, 7, 8],
      [0, 3, 6], [1, 4, 7], [2, 5, 8],
      [0, 4, 8], [2, 4, 6]
    ];

    function makeMove(index) {
      if (board[index] !== '' || !gameActive) return;

      board[index] = player;
      updateBoard();

      if (checkWin(player)) {
        document.getElementById('status').innerText = 'Você venceu! 🎉';
        document.getElementById('status').style.color = '#10b981';
        gameActive = false;
        window.parent.postMessage({ game: 'tictactoe', result: 'win' }, '*');
        return;
      }

      if (board.every(cell => cell !== '')) {
        document.getElementById('status').innerText = 'Empate! 🤝';
        document.getElementById('status').style.color = '#eab308';
        gameActive = false;
        window.parent.postMessage({ game: 'tictactoe', result: 'draw' }, '*');
        return;
      }

      document.getElementById('status').innerText = 'Mika está jogando...';
      gameActive = false;
      const moveGeneration = generation;
      mikaMoveTimeoutId = setTimeout(() => mikaMove(moveGeneration), 500);
    }

    function mikaMove(moveGeneration) {
      if (moveGeneration !== generation) return;

      // Find empty indices
      const empties = board.map((val, idx) => val === '' ? idx : null).filter(val => val !== null);
      if (empties.length === 0) return;

      // Smart AI: 1. check if can win, 2. check if can block player, 3. random
      let chosenMove = -1;

      // Check win
      for (const cond of winConditions) {
        const vals = cond.map(idx => board[idx]);
        if (vals.filter(v => v === mika).length === 2 && vals.includes('')) {
          chosenMove = cond[vals.indexOf('')];
          break;
        }
      }

      // Check block
      if (chosenMove === -1) {
        for (const cond of winConditions) {
          const vals = cond.map(idx => board[idx]);
          if (vals.filter(v => v === player).length === 2 && vals.includes('')) {
            chosenMove = cond[vals.indexOf('')];
            break;
          }
        }
      }

      // Random
      if (chosenMove === -1) {
        chosenMove = empties[Math.floor(Math.random() * empties.length)];
      }

      board[chosenMove] = mika;
      updateBoard();

      if (checkWin(mika)) {
        document.getElementById('status').innerText = 'Mika venceu! 😈';
        document.getElementById('status').style.color = '#ef4444';
        gameActive = false;
        window.parent.postMessage({ game: 'tictactoe', result: 'lose' }, '*');
        return;
      }

      if (board.every(cell => cell !== '')) {
        document.getElementById('status').innerText = 'Empate! 🤝';
        document.getElementById('status').style.color = '#eab308';
        gameActive = false;
        window.parent.postMessage({ game: 'tictactoe', result: 'draw' }, '*');
        return;
      }

      document.getElementById('status').innerText = 'Sua vez! (X)';
      gameActive = true;
    }

    function checkWin(sym) {
      return winConditions.some(condition => {
        return condition.every(index => board[index] === sym);
      });
    }

    function updateBoard() {
      const cells = document.querySelectorAll('.cell');
      board.forEach((val, idx) => {
        cells[idx].innerText = val;
        cells[idx].className = 'cell ' + (val ? val.toLowerCase() : '');
      });
    }

    function resetBoard() {
      clearTimeout(mikaMoveTimeoutId);
      mikaMoveTimeoutId = null;
      generation++;
      board = ['', '', '', '', '', '', '', '', ''];
      gameActive = true;
      document.getElementById('status').innerText = 'Sua vez! (X)';
      document.getElementById('status').style.color = '#f472b6';
      updateBoard();
    }
"#;
