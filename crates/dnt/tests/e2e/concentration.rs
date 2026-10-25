use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;

#[test]
fn concentration_on_a_spell_ends() {
    let mut t = Tui::scenario("concentration.end", Home::party());
    open(&mut t, "Ranger");
    t.send(TAB).send(TAB).wait("prepared 0/1");
    t.send("C");
    wait_status(&mut t, "Not concentrating");
    // Prepare Ward of Thorns and cast it.
    t.send("a").wait("add a spell");
    t.send("Ward of Thorns").send(ENTER);
    wait_status(&mut t, "Added Ward of Thorns");
    t.send("j").send(ENTER).wait("Cast Ward of Thorns");
    t.send(ENTER);
    wait_status(&mut t, "Cast Ward of Thorns");
    t.wait("◎ concentrating on Ward of Thorns");
    t.send("C");
    wait_status(&mut t, "Stopped concentrating on Ward of Thorns");
    t.wait_gone("◎ concentrating");
    // The slot stays spent.
    t.wait("Slots  1 ●○");
}
