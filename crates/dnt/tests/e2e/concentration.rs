use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;

#[test]
fn concentration_on_a_spell_ends() {
    let mut t = Tui::scenario("concentration.end", Home::party());
    open(&mut t, "Ranger");
    t.send(TAB).send(TAB).wait("prepared 0/2");
    t.send("C");
    wait_status(&mut t, "Not concentrating");
    // Prepare Fog Cloud and cast it.
    t.send("a").wait("add a spell");
    t.send("Fog Cloud").send(ENTER);
    wait_status(&mut t, "Added Fog Cloud");
    t.send("j").send(ENTER).wait("Cast Fog Cloud");
    t.send(ENTER);
    wait_status(&mut t, "Cast Fog Cloud");
    t.wait("◎ concentrating on Fog Cloud");
    t.send("C");
    wait_status(&mut t, "Stopped concentrating on Fog Cloud");
    t.wait_gone("◎ concentrating");
    // The slot stays spent.
    t.wait("Slots  1 ●○");
}
