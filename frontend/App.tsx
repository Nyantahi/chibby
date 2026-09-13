import { lazy, Suspense } from 'react';
import { Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Projects from './components/Projects';
import AgentDrawer from './components/AgentDrawer';

// Layout, the index route (Projects) and the always-mounted AgentDrawer stay
// eager — they are on screen at first paint, so lazy-loading them would only
// add a suspense flash. Every other route is split into its own chunk and
// fetched on navigation, which is what keeps the entry bundle small.
const ProjectDetail = lazy(() => import('./components/ProjectDetail'));
const AddProject = lazy(() => import('./components/AddProject'));
const RunDetail = lazy(() => import('./components/RunDetail'));
const Insights = lazy(() => import('./components/Insights'));
const Templates = lazy(() => import('./components/Templates'));
const Settings = lazy(() => import('./components/Settings'));
const CrashLog = lazy(() => import('./components/CrashLog'));

function App() {
  return (
    <>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Projects />} />
          <Route path="projects" element={<Projects />} />
          <Route
            path="project/:projectId"
            element={
              <Suspense fallback={<RouteFallback />}>
                <ProjectDetail />
              </Suspense>
            }
          />
          <Route
            path="add-project"
            element={
              <Suspense fallback={<RouteFallback />}>
                <AddProject />
              </Suspense>
            }
          />
          <Route
            path="run/:runId"
            element={
              <Suspense fallback={<RouteFallback />}>
                <RunDetail />
              </Suspense>
            }
          />
          <Route
            path="insights"
            element={
              <Suspense fallback={<RouteFallback />}>
                <Insights />
              </Suspense>
            }
          />
          <Route
            path="templates"
            element={
              <Suspense fallback={<RouteFallback />}>
                <Templates />
              </Suspense>
            }
          />
          <Route
            path="settings"
            element={
              <Suspense fallback={<RouteFallback />}>
                <Settings />
              </Suspense>
            }
          />
          <Route
            path="crashes"
            element={
              <Suspense fallback={<RouteFallback />}>
                <CrashLog />
              </Suspense>
            }
          />
        </Route>
      </Routes>
      <AgentDrawer />
    </>
  );
}

/** Shown while a route's chunk is being fetched. */
function RouteFallback() {
  return <div className="loading">Loading…</div>;
}

export default App;
