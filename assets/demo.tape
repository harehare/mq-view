Output "demo.gif"

Require mq-view

Set Shell "zsh"
Set FontSize 14
Set Width 1200
Set Height 800
Set TypingSpeed 50ms
Set FontFamily "Hack Nerd Font"
Set Padding 16
Set WindowBar Colorful
Set Framerate 20

Hide
Type "clear"
Enter
Show

# Open the document in the pager
Type `mq-view --pager demo.md`
Enter
Sleep 3s

# Heading outline
Tab
Sleep 1s
Down
Down
Sleep 600ms
Escape
Sleep 500ms

# Link list: jump to an in-document #anchor
Enter
Sleep 1.5s
Down
Sleep 500ms
Down
Sleep 1s
Enter
Sleep 2s

# Link list: follow a link to another Markdown file
Type "g"
Sleep 500ms
Enter
Sleep 1s
Down
Sleep 500ms
Down
Sleep 500ms
Down
Sleep 1s
Enter
Sleep 2.5s

# Go back and forward through the followed links
Type "["
Sleep 2s
Type "]"
Sleep 2s
Type "["
Sleep 2s

Type "q"
Sleep 500ms
