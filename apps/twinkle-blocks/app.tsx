/// <reference path="../../../twinkle/crates/twinkle-jsx-runtime/types/twinkle.d.ts" />
import './app.css';

const act = (action: string) => twinkle.request({action});

export default function App() {
    const board = (twinkle.data.board as string[]) || [];
    const cell = (color: string, index: number) =>
        <Div key={index} className={color ? 'cell filled ' + color : 'cell'} />;
    return <Window id="main" width={800} height={720} title="Twinkle Blocks" background={0xff101820}>
        <Column className="app">
            <Row className="topbar">
                <Text className="brand">Twinkle Blocks</Text>
                <Text>{'Score ' + twinkle.data.score + '  ·  Lines ' + twinkle.data.lines + '  ·  Level ' + twinkle.data.level}</Text>
                <Button className="action" id="game-menu-button" onClick={() => act('menu')}>Game menu</Button>
                <Menu id="game-menu" anchor="game-menu-button" open={Boolean(twinkle.data.menuOpen)}>
                    <MenuItem id="toggle" label={twinkle.data.paused ? 'Start' : 'Pause'} onClick={() => act('toggle')} />
                    <MenuItem id="restart" label="New game" onClick={() => act('restart')} />
                </Menu>
            </Row>
            <Row className="play">
                <div className="board" role="group" aria-label="Game board">{board.map(cell)}</div>
                <Column className="side">
                    <Text className={twinkle.data.over ? 'status game-over' : 'status'}>{String(twinkle.data.message || '')}</Text>
                    <Text>Next</Text>
                    <Div className="next-piece"><Div className="next-block" /></Div>
                    <Text>Controls</Text>
                    <Button className="action" id="move-left" onClick={() => act('left')}>Move left</Button>
                    <Button className="action" id="move-right" onClick={() => act('right')}>Move right</Button>
                    <Button className="action" id="rotate" onClick={() => act('rotate')}>Rotate</Button>
                    <Button className="action" id="soft-drop" onClick={() => act('down')}>Soft drop</Button>
                    <Button className="action" id="hard-drop" onClick={() => act('drop')}>Hard drop</Button>
                    <Text className="hint">D-pad / left stick: move and drop · A: rotate · B: hard drop · Start: game menu. Keyboard: ↑/↓, Alt+←/→, Enter, Escape.</Text>
                </Column>
            </Row>
        </Column>
    </Window>;
}
