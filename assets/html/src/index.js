import './pty.css'
import '@xterm/xterm/css/xterm.css'


let { Terminal } = require('@xterm/xterm')

let term = new Terminal();


console.log(document.getElementById("terminal"))

term.open(document.getElementById('terminal'))

term.onData(() => {

})