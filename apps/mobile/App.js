// Phone shell for the emulator.
//
// Expo Go cannot load the compiled Rust core as a native library, so the app
// shows the browser build (the same WebAssembly core CI tests) inside a web
// view. When the iOS app moves to a real native build, this screen is where
// the Swift/Rust core will plug in instead.

import { StatusBar } from 'expo-status-bar';
import { useRef, useState } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';
import { WebView } from 'react-native-webview';

// The diagnostics page published from apps/browser by .github/workflows/pages.yml
const CORE_URL = 'https://wulfnout.github.io/ps5emu/';

const C = { bg: '#0b0d12', text: '#e6e9f0', dim: '#8a93a8', accent: '#6ea8ff', bad: '#ff5d6c' };

export default function App() {
  const web = useRef(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const retry = () => {
    setError(null);
    setLoading(true);
    web.current?.reload();
  };

  return (
    <SafeAreaProvider>
      <SafeAreaView style={styles.root} edges={['top', 'bottom']}>
        <StatusBar style="light" />
        {error ? (
          <View style={styles.center}>
            <Text style={styles.title}>Couldn't load the emulator core</Text>
            <Text style={styles.body}>{error}</Text>
            <Text style={styles.body}>Check your connection, then try again.</Text>
            <Pressable style={styles.button} onPress={retry}>
              <Text style={styles.buttonText}>Try again</Text>
            </Pressable>
          </View>
        ) : (
          <WebView
            ref={web}
            source={{ uri: CORE_URL }}
            style={styles.web}
            originWhitelist={['https://*']}
            pullToRefreshEnabled
            allowsBackForwardNavigationGestures
            setSupportMultipleWindows={false}
            onLoadEnd={() => setLoading(false)}
            onError={(e) => setError(e.nativeEvent.description || 'Network error')}
            onHttpError={(e) => setError(`The page returned HTTP ${e.nativeEvent.statusCode}`)}
          />
        )}
        {loading && !error && (
          <View style={[StyleSheet.absoluteFill, styles.center]} pointerEvents="none">
            <ActivityIndicator color={C.accent} size="large" />
            <Text style={styles.dimText}>Loading emulator core…</Text>
          </View>
        )}
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: C.bg },
  web: { flex: 1, backgroundColor: C.bg },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24, backgroundColor: C.bg },
  title: { color: C.bad, fontSize: 18, fontWeight: '600', marginBottom: 10, textAlign: 'center' },
  body: { color: C.text, fontSize: 15, textAlign: 'center', marginBottom: 6 },
  dimText: { color: C.dim, marginTop: 12 },
  button: { marginTop: 16, borderColor: C.accent, borderWidth: 1, borderRadius: 10, paddingVertical: 10, paddingHorizontal: 22 },
  buttonText: { color: C.accent, fontSize: 16, fontWeight: '600' },
});
