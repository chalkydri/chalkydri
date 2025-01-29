import 'dart:io';

import 'package:chalkydri_manager/devices.dart';
import 'package:chalkydri_manager/menu.dart';
import 'package:chalkydri_manager/utils.dart';
import 'package:flutter/material.dart';

String? edition;

void main() {
  if (Platform.isWindows || Platform.isLinux) {
    edition = "DS";
  } else {
    edition = "Web";
  }

  runApp(const App());
}

class App extends StatelessWidget {
  const App({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Chalkydri Manager',
      theme: ThemeData(
        colorScheme: const ColorScheme.dark(primary: Colors.deepPurple),
        useMaterial3: true,
      ),
      initialRoute: '/',
      routes: <String, Widget Function(BuildContext)>{
        '/': (_) {
          return const Home();
        },
				if (isDS()) '/devices': (_) {
					return const ChalkydriDevices();
				},
      },
    );
  }
}

class Home extends StatefulWidget {
  const Home({super.key});

  // This widget is the home page of your application. It is stateful, meaning
  // that it has a State object (defined below) that contains fields that affect
  // how it looks.

  // This class is the configuration for the state. It holds the values (in this
  // case the title) provided by the parent (in this case the App widget) and
  // used by the build method of the State. Fields in a Widget subclass are
  // always marked "final".

  @override
  State<Home> createState() => _HomeState();
}

class _HomeState extends State<Home> {
  @override
  Widget build(BuildContext context) {
    // This method is rerun every time setState is called, for instance as done
    // by the _incrementCounter method above.
    //
    // The Flutter framework has been optimized to make rerunning build methods
    // fast, so that you can just rebuild anything that needs updating rather
    // than having to individually change instances of widgets.

    return Scaffold(
      appBar: buildAppBar(context),
      drawer: ChalkydriMenu(),
      body: Center(
        // Center is a layout widget. It takes a single child and positions it
        // in the middle of the parent.
        child: Column(
          // Column is also a layout widget. It takes a list of children and
          // arranges them vertically. By default, it sizes itself to fit its
          // children horizontally, and tries to be as tall as its parent.
          //
          // Column has various properties to control how it sizes itself and
          // how it positions its children. Here we use mainAxisAlignment to
          // center the children vertically; the main axis here is the vertical
          // axis because Columns are vertical (the cross axis would be
          // horizontal).
          //
          // TRY THIS: Invoke "debug painting" (choose the "Toggle Debug Paint"
          // action in the IDE, or press "p" in the console), to see the
          // wireframe for each widget.
          mainAxisAlignment: MainAxisAlignment.center,
          children: <Widget>[
            const Text(
              textScaler: TextScaler.linear(2.0),
              style: TextStyle(fontWeight: FontWeight.bold),
              'Chalkydri',
            ),
            Text('Camera health'),
            Table(
              children: [
                TableRow(
                  children: [
                    const Text('Battery'),
                    Text('91%'),
                  ],
                ),
                TableRow(
                  children: [
                    const Text('Uptime'),
                    Text('2 days 9hours'),
                  ],
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

PreferredSizeWidget buildAppBar(BuildContext context) {
  return AppBar(
    //leading: IconButton(
    //	onPressed: () {
    //	},
    //	icon: Icons.menu,
    //),
    // TRY THIS: Try changing the color here to a specific color (to
    // Colors.amber, perhaps?) and trigger a hot reload to see the AppBar
    // change color while the other colors stay the same.
    backgroundColor: Theme.of(context).colorScheme.inversePrimary,
    // Here we take the value from the MyHomePage object that was created by
    // the App.build method, and use it to set our appbar title.
    title: Row(children: [
      const Text(
        style: TextStyle(fontWeight: FontWeight.bold),
        'Chalkydri Manager ',
      ),
      Text(edition!),
    ]),
  );
}

Widget buildScaffold(BuildContext context, Widget? widget) {
  return Scaffold(
    appBar: buildAppBar(context),
    drawer: const ChalkydriMenu(),
    body: widget,
  );
}
